import CoreGraphics
import Foundation

/// Turns the dataset's relative dates into absolute ones, anchored at the start of the
/// seeding day in the guest's time zone.
struct RelativeDates {
    let calendar = Calendar.current
    let today: Date

    init(now: Date = Date()) {
        today = Calendar.current.startOfDay(for: now)
    }

    func day(_ offset: Int) -> Date {
        calendar.date(byAdding: .day, value: offset, to: today)!
    }

    /// `time` is local "HH:MM"; nil means the start of that day.
    func date(day offset: Int, time: String?) -> Date {
        let start = day(offset)
        guard let time else { return start }
        let parts = time.split(separator: ":").compactMap { Int($0) }
        precondition(parts.count == 2, "time must be HH:MM, got \(time)")
        return calendar.date(bySettingHour: parts[0], minute: parts[1], second: 0, of: start)!
    }

    func components(day offset: Int, time: String?) -> DateComponents {
        var parts: Set<Calendar.Component> = [.year, .month, .day]
        if time != nil { parts.formUnion([.hour, .minute]) }
        return calendar.dateComponents(parts, from: date(day: offset, time: time))
    }
}

extension CGColor {
    /// "#RRGGBB"
    static func hex(_ string: String) -> CGColor {
        let value = UInt32(string.dropFirst(), radix: 16) ?? 0
        return CGColor(
            srgbRed: CGFloat((value >> 16) & 0xFF) / 255,
            green: CGFloat((value >> 8) & 0xFF) / 255,
            blue: CGFloat(value & 0xFF) / 255,
            alpha: 1
        )
    }
}
