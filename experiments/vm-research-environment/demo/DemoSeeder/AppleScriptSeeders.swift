import Foundation

/// Notes has no public framework for creating notes, and Contacts has none for choosing
/// the "My Card" contact, so both go through each app's scripting dictionary. macOS asks
/// once for permission to control each app.

struct NotesSeeder {
    let dataset: NotesFile

    func run() throws {
        let account = try runAppleScript("tell application \"Notes\" to get name of default account")
        var script = "tell application \"Notes\"\n"
        script += "set theAccount to account \(quoted(account))\n"
        for folder in dataset.folders {
            script += "if exists folder \(quoted(folder.name)) of theAccount then delete folder \(quoted(folder.name)) of theAccount\n"
            script += "set theFolder to make new folder at theAccount with properties {name:\(quoted(folder.name))}\n"
            // Notes lists the newest note first, so the folder's first note is made last.
            for note in folder.notes.reversed() {
                script += "make new note at theFolder with properties {body:\(quoted(note.html))}\n"
            }
        }
        script += "end tell\n"
        _ = try runAppleScript(script)
        log("notes: \(dataset.folders.map(\.notes.count).reduce(0, +)) notes in \(dataset.folders.count) folders of '\(account)'")
    }
}

/// Makes the persona's contact the My Card and deletes the card it replaces, which
/// Setup Assistant created from the account's original name.
struct MyCardSeeder {
    let me: Contact

    func run() throws {
        let script = """
        tell application "Contacts"
            set persona to first person whose first name is \(quoted(me.given ?? "")) and last name is \(quoted(me.family ?? ""))
            set replaced to ""
            try
                set setupCard to my card
                if id of setupCard is not id of persona then
                    set replaced to name of setupCard
                    delete setupCard
                end if
            end try
            set my card to persona
            save
            return replaced
        end tell
        """
        let replaced = try runAppleScript(script)
        log("contacts: My Card is \(me.displayName)" + (replaced.isEmpty ? "" : ", replacing '\(replaced)'"))
    }
}

func quoted(_ text: String) -> String {
    "\"" + text.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
}

func runAppleScript(_ source: String) throws -> String {
    var error: NSDictionary?
    guard let result = NSAppleScript(source: source)?.executeAndReturnError(&error), error == nil else {
        throw SeederError.script(error?[NSAppleScript.errorMessage] as? String ?? "unknown AppleScript error")
    }
    return result.stringValue ?? ""
}
