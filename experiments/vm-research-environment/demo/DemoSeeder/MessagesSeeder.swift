import Foundation
import SQLite3

/// Writes the dataset's conversations straight into ~/Library/Messages/chat.db, which is
/// the only way to give Messages history without an Apple Account. Messages, imagent and
/// IMDPersistenceAgent must not be running, and the caller needs Full Disk Access.
///
/// Messages draws a bubble from `attributedBody` (an NSArchiver typedstream of an
/// attributed string), not from `text`, so both are written. A photo is an attachment
/// file under ~/Library/Messages/Attachments referenced from the attributed string.
struct MessagesSeeder {
    let dataset: MessagesFile
    let contacts: [String: Contact]
    let me: Contact
    let photos: TaggedPhotos
    let dates: RelativeDates

    private let home = FileManager.default.homeDirectoryForCurrentUser
    private let accountGUID = UUID().uuidString

    func run() throws {
        let database = try ChatDatabase(path: home.appendingPathComponent("Library/Messages/chat.db").path)
        guard try database.count("message") == 0 else {
            throw SeederError.invalid("chat.db already holds messages; seed Messages into an empty database")
        }
        try database.execute("BEGIN")
        var handles: [String: Int64] = [:]
        for id in Set(dataset.conversations.flatMap(\.participants)) {
            guard let handle = contacts[id]?.messagesHandle else { throw SeederError.invalid("no phone for '\(id)'") }
            handles[id] = try database.insert("handle", [
                "id": handle, "country": "us", "service": "iMessage", "uncanonicalized_id": handle,
            ])
        }
        var total = 0
        for conversation in dataset.conversations {
            total += try seed(conversation, handles: handles, into: database)
        }
        try database.execute("COMMIT")
        log("messages: \(total) messages in \(dataset.conversations.count) conversations")
    }

    private func seed(_ conversation: MessagesFile.Conversation, handles: [String: Int64], into database: ChatDatabase) throws -> Int {
        let isGroup = conversation.participants.count > 1
        let identifier = isGroup
            ? "chat" + String(UInt64.random(in: 100_000_000_000_000_000...999_999_999_999_999_999))
            : contacts[conversation.participants[0]]!.messagesHandle!
        let ordered = conversation.messages.sorted { date(of: $0) < date(of: $1) }
        let unreadFrom = unreadTail(of: ordered)

        let chat = try database.insert("chat", [
            "guid": "iMessage;\(isGroup ? "+" : "-");\(identifier)", "style": isGroup ? 43 : 45, "state": 3,
            "chat_identifier": identifier, "service_name": "iMessage",
            "room_name": isGroup ? identifier : nil, "display_name": conversation.name ?? "",
            "group_id": UUID().uuidString, "account_login": "E:" + (me.emails.first?.address ?? ""),
            "last_addressed_handle": me.messagesHandle ?? "", "is_archived": 0, "is_filtered": 0,
            "successful_query": 1,
        ])
        for id in conversation.participants {
            _ = try database.insert("chat_handle_join", ["chat_id": chat, "handle_id": handles[id]!])
        }

        for (index, message) in ordered.enumerated() {
            let sent = date(of: message)
            let fromMe = message.from == "me"
            let isRead = fromMe || index < unreadFrom
            var text = message.text
            var attachment: (guid: String, file: URL)?
            if let tag = message.photo {
                let file = try photos.take(tag)
                attachment = (UUID().uuidString, file)
                text = "\u{FFFC}" + message.text
            }
            let row = try database.insert("message", [
                "guid": UUID().uuidString, "text": text,
                "attributedBody": archivedBody(text, attachmentGUID: attachment?.guid),
                "handle_id": fromMe ? (isGroup ? 0 : handles[conversation.participants[0]]!) : handles[message.from]!,
                "service": "iMessage", "account": "E:" + (me.emails.first?.address ?? ""),
                "account_guid": accountGUID, "date": nanoseconds(sent),
                "date_read": isRead ? nanoseconds(sent.addingTimeInterval(90)) : 0,
                "date_delivered": fromMe ? nanoseconds(sent.addingTimeInterval(2)) : 0,
                "is_delivered": 1, "is_finished": 1, "is_from_me": fromMe ? 1 : 0, "is_read": isRead ? 1 : 0,
                "is_sent": fromMe ? 1 : 0, "is_prepared": 1, "version": 10, "type": 0, "item_type": 0,
                "cache_has_attachments": attachment == nil ? 0 : 1,
            ])
            _ = try database.insert("chat_message_join", ["chat_id": chat, "message_id": row, "message_date": nanoseconds(sent)])
            if let attachment {
                let stored = try storeAttachment(attachment.file, guid: attachment.guid)
                let attachmentRow = try database.insert("attachment", [
                    "guid": attachment.guid, "original_guid": attachment.guid,
                    "created_date": Int64(sent.timeIntervalSinceReferenceDate),
                    "start_date": 0, "filename": stored.path, "uti": "public.jpeg", "mime_type": "image/jpeg",
                    "transfer_state": 5, "is_outgoing": fromMe ? 1 : 0, "transfer_name": stored.name,
                    "total_bytes": stored.bytes, "is_sticker": 0, "hide_attachment": 0,
                ])
                _ = try database.insert("message_attachment_join", ["message_id": row, "attachment_id": attachmentRow])
            }
        }
        return ordered.count
    }

    /// Incoming messages after the last message Mia sent today stay unread, as a phone
    /// in someone's pocket would show them.
    private func unreadTail(of messages: [MessagesFile.Message]) -> Int {
        guard let last = messages.last, last.from != "me", last.day == 0 else { return messages.count }
        let lastMine = messages.lastIndex { $0.from == "me" } ?? -1
        return lastMine + 1
    }

    private func date(of message: MessagesFile.Message) -> Date {
        dates.date(day: message.day, time: message.time)
    }

    private func nanoseconds(_ date: Date) -> Int64 {
        Int64(date.timeIntervalSinceReferenceDate * 1_000_000_000)
    }

    /// The attachment as iMessage would deliver it: a JPEG at most 2048 pixels on its long
    /// side, in Messages' own attachment tree, named with the "~/" path Messages records.
    private func storeAttachment(_ source: URL, guid: String) throws -> (path: String, name: String, bytes: Int) {
        let hex = guid.replacingOccurrences(of: "-", with: "").lowercased()
        let relative = "Library/Messages/Attachments/\(hex.prefix(2))/\(hex.dropFirst(2).prefix(2))/\(guid)"
        let directory = home.appendingPathComponent(relative)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let name = "IMG_\(Int.random(in: 1000...9999)).jpeg"
        let data = try thumbnailJPEG(of: source, maxPixels: 2048)
        try data.write(to: directory.appendingPathComponent(name))
        return ("~/\(relative)/\(name)", name, data.count)
    }

    /// Messages displays a row only from `attributedBody`, which it reads as a typedstream
    /// archive; the deprecated `NSArchiver` is the only public writer of that format.
    private func archivedBody(_ text: String, attachmentGUID: String?) -> Data {
        let body = NSMutableAttributedString(string: text)
        let part = NSAttributedString.Key("__kIMMessagePartAttributeName")
        if let attachmentGUID {
            body.addAttributes([
                part: NSNumber(value: 0),
                NSAttributedString.Key("__kIMFileTransferGUIDAttributeName"): attachmentGUID,
            ], range: NSRange(location: 0, length: 1))
            if body.length > 1 {
                body.addAttributes([part: NSNumber(value: 1)], range: NSRange(location: 1, length: body.length - 1))
            }
        } else {
            body.addAttributes([part: NSNumber(value: 0)], range: NSRange(location: 0, length: body.length))
        }
        return NSArchiver.archivedData(withRootObject: body)
    }
}

/// Photo files picked by the dataset's photo tags, each used once.
final class TaggedPhotos {
    static let tokens = [
        "LA": "demo-la", "Sicily": "sicily", "Tahoe": "tahoe", "ValleyOfFire": "valleyoffire",
        "MexicoCity": "mexicocity", "India": "india", "Marin": "marin", "Amalfi": "amalfi",
        "SoCal": "socalroadtrip",
    ]
    private var remaining: [URL]

    init(_ files: [URL]) {
        remaining = files.filter { !["mov", "m4v", "png"].contains($0.pathExtension.lowercased()) }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
    }

    func take(_ tag: String) throws -> URL {
        guard let token = Self.tokens[tag] else { throw SeederError.invalid("unknown photo tag '\(tag)'") }
        guard let index = remaining.firstIndex(where: { $0.lastPathComponent.lowercased().contains(token) }) else {
            throw SeederError.invalid("no unused photo for tag '\(tag)'")
        }
        return remaining.remove(at: index)
    }
}
