import Foundation

/// Decodes every dataset file and checks the references between them, so a mistake in
/// the data stops the seeding before any app's store has been written.
struct DatasetCheck {
    let data: URL

    func run() throws {
        let contacts = try Dataset.load(ContactsFile.self, "contacts.json", from: data)
        let calendar = try Dataset.load(CalendarFile.self, "calendar.json", from: data)
        let reminders = try Dataset.load(RemindersFile.self, "reminders.json", from: data)
        let notes = try Dataset.load(NotesFile.self, "notes.json", from: data)
        let messages = try Dataset.load(MessagesFile.self, "messages.json", from: data)
        let documents = try Dataset.load(DocumentsFile.self, "documents.json", from: data)
        let portraits = try Dataset.load([String: String].self, "contact-photos.json", from: data)

        var problems: [String] = []
        let ids = Set(contacts.contacts.map(\.id))
        if !ids.contains(contacts.me) { problems.append("contacts.me '\(contacts.me)' is not a contact") }
        for contact in contacts.contacts where contact.photo && portraits[contact.id] == nil {
            problems.append("contact '\(contact.id)' wants a photo but contact-photos.json has none")
        }
        let calendars = Set(calendar.calendars.map(\.title))
        for event in calendar.events where !calendars.contains(event.calendar) {
            problems.append("event '\(event.title)' names unknown calendar '\(event.calendar)'")
        }
        for conversation in messages.conversations {
            for id in conversation.participants where !ids.contains(id) || contacts.contacts.first(where: { $0.id == id })?.messagesHandle == nil {
                problems.append("conversation '\(conversation.id)' has participant '\(id)' without a contact phone")
            }
            for message in conversation.messages {
                if message.from != "me", !conversation.participants.contains(message.from) {
                    problems.append("conversation '\(conversation.id)' has a message from non-participant '\(message.from)'")
                }
                if let tag = message.photo, TaggedPhotos.tokens[tag] == nil {
                    problems.append("conversation '\(conversation.id)' uses unknown photo tag '\(tag)'")
                }
            }
        }
        for document in documents.documents where !["pdf", "rtf", "docx", "txt", "csv"].contains(document.format) {
            problems.append("document '\(document.path)' has unknown format '\(document.format)'")
        }
        guard problems.isEmpty else { throw SeederError.invalid(problems.joined(separator: "; ")) }
        log("check: \(contacts.contacts.count) contacts, \(calendar.events.count) events, "
            + "\(reminders.lists.map(\.items.count).reduce(0, +)) reminders, "
            + "\(notes.folders.map(\.notes.count).reduce(0, +)) notes, "
            + "\(messages.conversations.map(\.messages.count).reduce(0, +)) messages, \(documents.documents.count) documents")
    }
}
