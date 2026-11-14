import Foundation

// The JSON files in demo/data. Every date is relative to the seeding day: `day` is an
// offset in days from today (negative is the past), `time` is local "HH:MM".

struct ContactsFile: Decodable {
    let me: String
    let contacts: [Contact]
}

struct Contact: Decodable {
    struct Phone: Decodable { let label: String; let number: String }
    struct Email: Decodable { let label: String; let address: String }
    struct Address: Decodable {
        let label: String
        let street: String
        let city: String
        let state: String
        let postalCode: String
        let country: String
    }
    /// A birthday whose month and day are null is placed relative to the seeding day.
    struct Birthday: Decodable { let month: Int?; let day: Int?; let year: Int? }

    let id: String
    let given: String?
    let family: String?
    let organization: String?
    let jobTitle: String?
    let phones: [Phone]
    let emails: [Email]
    let address: Address?
    let birthday: Birthday?
    let groups: [String]
    let note: String?
    let photo: Bool
    let isCompany: Bool

    var displayName: String {
        if isCompany { return organization ?? id }
        return [given, family].compactMap { $0 }.joined(separator: " ")
    }

    /// The handle Messages stores for this contact's first phone: E.164, digits only.
    var messagesHandle: String? {
        guard let number = phones.first?.number else { return nil }
        return "+" + number.filter(\.isNumber)
    }
}

struct CalendarFile: Decodable {
    struct CalendarSpec: Decodable { let title: String; let color: String }
    struct Recurrence: Decodable {
        let frequency: String
        let interval: Int?
        let daysOfWeek: [String]?
        let untilDay: Int?
    }
    struct Event: Decodable {
        let calendar: String
        let title: String
        let day: Int
        let start: String?
        let end: String?
        let allDay: Bool
        let days: Int?
        let location: String?
        let notes: String?
        let url: String?
        let alarmMinutes: Int?
        let recurrence: Recurrence?
    }

    let calendars: [CalendarSpec]
    let events: [Event]
}

struct RemindersFile: Decodable {
    struct Item: Decodable {
        let title: String
        let notes: String?
        let dueDay: Int?
        let dueTime: String?
        let completed: Bool
        let priority: Int
    }
    struct List: Decodable { let title: String; let color: String; let items: [Item] }

    let lists: [List]
}

struct NotesFile: Decodable {
    struct Note: Decodable { let title: String; let html: String }
    struct Folder: Decodable { let name: String; let notes: [Note] }

    let folders: [Folder]
}

struct MessagesFile: Decodable {
    struct Message: Decodable {
        let from: String
        let day: Int
        let time: String
        let text: String
        let photo: String?
    }
    struct Conversation: Decodable {
        let id: String
        let participants: [String]
        let name: String?
        let messages: [Message]
    }

    let conversations: [Conversation]
}

struct DocumentsFile: Decodable {
    struct Document: Decodable {
        let path: String
        let format: String
        let title: String
        let body: String
    }

    let documents: [Document]
}

enum Dataset {
    static func load<T: Decodable>(_ type: T.Type, _ name: String, from directory: URL) throws -> T {
        let data = try Data(contentsOf: directory.appendingPathComponent(name))
        return try JSONDecoder().decode(T.self, from: data)
    }
}
