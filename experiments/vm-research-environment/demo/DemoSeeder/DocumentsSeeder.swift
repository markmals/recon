import AppKit
import Foundation

/// Writes the dataset's documents into the home folder as real PDF, RTF, Word, text and
/// CSV files, with modification dates spread over the past few weeks.
struct DocumentsSeeder {
    let dataset: DocumentsFile
    let dates: RelativeDates

    private let home = FileManager.default.homeDirectoryForCurrentUser

    func run() throws {
        for (index, document) in dataset.documents.enumerated() {
            let url = home.appendingPathComponent(document.path).appendingPathExtension(document.format)
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try contents(of: document).write(to: url)
            let modified = dates.date(day: -2 - (index * 5) % 26, time: String(format: "%02d:%02d", 9 + index % 9, (index * 17) % 60))
            try FileManager.default.setAttributes([.modificationDate: modified, .creationDate: modified], ofItemAtPath: url.path)
        }
        log("documents: \(dataset.documents.count) files")
    }

    private func contents(of document: DocumentsFile.Document) throws -> Data {
        switch document.format {
        case "txt", "csv":
            return Data(document.body.utf8)
        case "rtf":
            let text = styled(document)
            return try text.data(from: NSRange(location: 0, length: text.length), documentAttributes: [.documentType: NSAttributedString.DocumentType.rtf])
        case "docx":
            let text = styled(document)
            return try text.data(from: NSRange(location: 0, length: text.length), documentAttributes: [.documentType: NSAttributedString.DocumentType.officeOpenXML])
        case "pdf":
            return pdf(styled(document))
        default:
            throw SeederError.invalid("unknown document format '\(document.format)'")
        }
    }

    /// A title, then paragraphs; lines starting with "- " become bullets.
    private func styled(_ document: DocumentsFile.Document) -> NSAttributedString {
        let result = NSMutableAttributedString()
        let body = NSMutableParagraphStyle()
        body.paragraphSpacing = 8
        body.lineHeightMultiple = 1.15
        result.append(NSAttributedString(string: document.title + "\n", attributes: [
            .font: NSFont.systemFont(ofSize: 22, weight: .semibold), .paragraphStyle: body,
        ]))
        for line in document.body.components(separatedBy: "\n") {
            let isBullet = line.hasPrefix("- ")
            let style = body.mutableCopy() as! NSMutableParagraphStyle
            if isBullet {
                style.headIndent = 18
                style.firstLineHeadIndent = 4
                style.paragraphSpacing = 2
            }
            result.append(NSAttributedString(
                string: (isBullet ? "•  " + line.dropFirst(2) : line) + "\n",
                attributes: [.font: NSFont.systemFont(ofSize: 12), .paragraphStyle: style]
            ))
        }
        return result
    }

    /// Lays the text out on US Letter pages with one-inch margins.
    private func pdf(_ text: NSAttributedString) -> Data {
        let page = CGRect(x: 0, y: 0, width: 612, height: 792)
        let frame = page.insetBy(dx: 72, dy: 72)
        let storage = NSTextStorage(attributedString: text)
        let layout = NSLayoutManager()
        storage.addLayoutManager(layout)

        let data = NSMutableData()
        var mediaBox = page
        let context = CGContext(consumer: CGDataConsumer(data: data)!, mediaBox: &mediaBox, nil)!
        NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: true)
        repeat {
            let container = NSTextContainer(size: frame.size)
            layout.addTextContainer(container)
            let glyphs = layout.glyphRange(for: container)
            context.beginPDFPage(nil)
            context.translateBy(x: 0, y: page.height)
            context.scaleBy(x: 1, y: -1)
            layout.drawBackground(forGlyphRange: glyphs, at: frame.origin)
            layout.drawGlyphs(forGlyphRange: glyphs, at: frame.origin)
            context.endPDFPage()
        } while NSMaxRange(layout.glyphRange(for: layout.textContainers.last!)) < layout.numberOfGlyphs
        context.closePDF()
        NSGraphicsContext.current = nil
        return data as Data
    }
}
