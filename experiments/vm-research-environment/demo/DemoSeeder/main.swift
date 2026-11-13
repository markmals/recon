import AppKit

// Seeds the demo persona's data into the logged-in guest account.
//
// usage: DemoSeeder STEP... --data DIR [--media DIR] [--portraits DIR]
//   steps: check contacts mycard calendar reminders photos notes documents messages
//
// Every step except `messages` uses an app's framework or scripting dictionary and must
// run from the app bundle launched in the GUI session (`open DemoSeeder.app --args …`),
// so macOS asks for, and records, permission for DemoSeeder itself. `messages` writes
// chat.db directly and needs Full Disk Access, which the guest's SSH sessions have.

enum SeederError: Error, CustomStringConvertible {
    case denied(String)
    case invalid(String)
    case unreadable(URL)
    case script(String)
    case sqlite(String)

    var description: String {
        switch self {
        case .denied(let what): "access to \(what) was not granted"
        case .invalid(let why): why
        case .unreadable(let url): "cannot read image \(url.path)"
        case .script(let message): "AppleScript failed: \(message)"
        case .sqlite(let message): "SQLite: \(message)"
        }
    }
}

/// Launched through `open`, the seeder has no terminal, so progress also goes to a log the
/// host reads back.
let logURL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Logs/DemoSeeder.log")

func log(_ line: String) {
    print(line)
    let stamped = "\(ISO8601DateFormatter().string(from: Date())) \(line)\n"
    if let handle = try? FileHandle(forWritingTo: logURL) {
        handle.seekToEndOfFile()
        handle.write(Data(stamped.utf8))
        try? handle.close()
    } else {
        try? Data(stamped.utf8).write(to: logURL)
    }
}

/// Danny's birthday is the all-day event on day +12 in calendar.json; his contact card
/// follows it so the two agree whenever the data is seeded.
let relativeBirthdays = ["danny": 12]

func option(_ name: String) -> URL? {
    guard let index = CommandLine.arguments.firstIndex(of: name), index + 1 < CommandLine.arguments.count else { return nil }
    return URL(fileURLWithPath: CommandLine.arguments[index + 1])
}

func files(in directory: URL?) throws -> [URL] {
    guard let directory else { return [] }
    return try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        .filter { !$0.lastPathComponent.hasPrefix(".") }
        .sorted { $0.lastPathComponent < $1.lastPathComponent }
}

func portraits(_ contacts: ContactsFile, data: URL, directory: URL?) throws -> [String: URL] {
    guard let directory else { return [:] }
    let names = try Dataset.load([String: String].self, "contact-photos.json", from: data)
    return names.mapValues { directory.appendingPathComponent($0) }
}

func seed(_ step: String, data: URL) async throws {
    let dates = RelativeDates()
    switch step {
    case "check":
        try DatasetCheck(data: data).run()
    case "contacts":
        let contacts = try Dataset.load(ContactsFile.self, "contacts.json", from: data)
        try await ContactsSeeder(
            dataset: contacts, portraits: try portraits(contacts, data: data, directory: option("--portraits")),
            dates: dates, relativeBirthdays: relativeBirthdays
        ).run()
    case "mycard":
        let contacts = try Dataset.load(ContactsFile.self, "contacts.json", from: data)
        try MyCardSeeder(me: contacts.contacts.first { $0.id == contacts.me }!).run()
    case "calendar":
        try await CalendarSeeder(dataset: try Dataset.load(CalendarFile.self, "calendar.json", from: data), dates: dates).run()
    case "reminders":
        try await RemindersSeeder(dataset: try Dataset.load(RemindersFile.self, "reminders.json", from: data), dates: dates).run()
    case "photos":
        try await PhotosSeeder(media: try files(in: option("--media")), dates: dates).run()
    case "notes":
        try NotesSeeder(dataset: try Dataset.load(NotesFile.self, "notes.json", from: data)).run()
    case "documents":
        try DocumentsSeeder(dataset: try Dataset.load(DocumentsFile.self, "documents.json", from: data), dates: dates).run()
    case "messages":
        let contacts = try Dataset.load(ContactsFile.self, "contacts.json", from: data)
        let byID = Dictionary(uniqueKeysWithValues: contacts.contacts.map { ($0.id, $0) })
        try MessagesSeeder(
            dataset: try Dataset.load(MessagesFile.self, "messages.json", from: data), contacts: byID,
            me: byID[contacts.me]!, photos: TaggedPhotos(try files(in: option("--media"))), dates: dates
        ).run()
    default:
        throw SeederError.invalid("unknown step '\(step)'")
    }
}

let arguments = Array(CommandLine.arguments.dropFirst())
let optionNames: Set = ["--data", "--media", "--portraits"]
let steps = arguments.enumerated().filter { index, argument in
    !argument.hasPrefix("--") && (index == 0 || !optionNames.contains(arguments[index - 1]))
}.map(\.element)

guard let data = option("--data"), !steps.isEmpty else {
    log("usage: DemoSeeder STEP... --data DIR [--media DIR] [--portraits DIR]")
    exit(2)
}
func runSteps() async -> Int32 {
    for step in steps {
        do {
            try await seed(step, data: data)
        } catch {
            log("FAILED \(step): \(error)")
            return 1
        }
    }
    log("done: \(steps.joined(separator: " "))")
    return 0
}

// tccd refuses to prompt for a faceless background process ("Policy disallows prompt"),
// so steps that need a privacy permission run inside a regular, active application.
if steps.allSatisfy({ ["check", "messages", "documents"].contains($0) }) {
    exit(await runSteps())
}
NSApplication.shared.setActivationPolicy(.regular)
Task { exit(await runSteps()) }
NSApp.activate()
NSApp.run()
