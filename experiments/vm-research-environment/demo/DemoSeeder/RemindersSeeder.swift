import EventKit
import Foundation

/// Creates the dataset's reminder lists in the local source and fills them. Lists with
/// the same titles are deleted first, so the step can be repeated.
struct RemindersSeeder {
    let dataset: RemindersFile
    let dates: RelativeDates

    func run() async throws {
        let store = EKEventStore()
        guard try await store.requestFullAccessToReminders() else { throw SeederError.denied("Reminders") }
        let source = try reminderSource(in: store)

        var count = 0
        for list in dataset.lists {
            for existing in store.calendars(for: .reminder) where existing.title == list.title {
                try store.removeCalendar(existing, commit: true)
            }
            let calendar = EKCalendar(for: .reminder, eventStore: store)
            calendar.title = list.title
            calendar.cgColor = .hex(list.color)
            calendar.source = source
            try store.saveCalendar(calendar, commit: true)

            for item in list.items {
                let reminder = EKReminder(eventStore: store)
                reminder.calendar = calendar
                reminder.title = item.title
                reminder.notes = item.notes
                reminder.priority = item.priority
                if let day = item.dueDay {
                    reminder.dueDateComponents = dates.components(day: day, time: item.dueTime)
                    if item.dueTime != nil {
                        reminder.addAlarm(EKAlarm(absoluteDate: dates.date(day: day, time: item.dueTime)))
                    }
                }
                if item.completed {
                    reminder.completionDate = dates.date(day: min(item.dueDay ?? -1, -1), time: "18:00")
                }
                try store.save(reminder, commit: false)
                count += 1
            }
            try store.commit()
        }
        log("reminders: \(count) items in \(dataset.lists.count) lists")
    }

    private func reminderSource(in store: EKEventStore) throws -> EKSource {
        if let source = store.defaultCalendarForNewReminders()?.source { return source }
        return try localSource(in: store)
    }
}
