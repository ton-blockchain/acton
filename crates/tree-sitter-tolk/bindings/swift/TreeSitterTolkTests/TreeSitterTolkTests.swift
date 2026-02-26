import XCTest
import SwiftTreeSitter
import TreeSitterTolk

final class TreeSitterTolkTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_tolk())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading Tolk grammar")
    }
}
