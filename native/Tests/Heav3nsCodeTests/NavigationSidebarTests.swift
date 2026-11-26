// [LAYER: PLUMBING]
import AppKit
import XCTest
@testable import Heav3nsCode

final class NavigationSidebarTests: XCTestCase {
    func testFolderSelectionIsStableUntilActivatedAndKeyboardParentRemainsAvailable() {
        _ = NSApplication.shared
        let sidebar = NavigationSidebar(frame: NSRect(x: 0, y: 0, width: 250, height: 500))
        func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
        let table = descendants(sidebar).compactMap { $0 as? KeyboardListView }.first!
        var opened = [String](), parents = 0
        sidebar.onOpen = { opened.append($0.id) }; sidebar.onBack = { parents += 1 }
        sidebar.show([SidebarEntry(id: "src", title: "src", detail: "", symbol: "folder", value: ["directory": true]), SidebarEntry(id: "README.md", title: "README.md", detail: "", symbol: "doc", value: [:])], location: "Project", message: "", canGoBack: true)
        table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        XCTAssertTrue(opened.isEmpty)
        func key(_ code: UInt16) {
            table.keyDown(with: NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: 0, context: nil, characters: "", charactersIgnoringModifiers: "", isARepeat: false, keyCode: code)!)
        }
        key(36); XCTAssertEqual(opened, ["src"])
        key(123); XCTAssertEqual(parents, 1)
        sidebar.previewsSelection = false
        table.selectRowIndexes(IndexSet(integer: 1), byExtendingSelection: false)
        XCTAssertEqual(opened, ["src"])
        key(36); XCTAssertEqual(opened, ["src", "README.md"])
        sidebar.loading("Loading…"); key(36); XCTAssertEqual(opened, ["src", "README.md"])
    }
    func testChangingFolderScopeClearsTheQueryAndCancelsItsPendingFilter() {
        _ = NSApplication.shared
        let sidebar = NavigationSidebar(frame: .zero)
        func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
        let search = descendants(sidebar).compactMap { $0 as? NSSearchField }.first!
        let filtered = expectation(description: "the previous folder query is not delivered"); filtered.isInverted = true
        sidebar.onFilter = { _ in filtered.fulfill() }
        search.stringValue = "parent-folder"
        sidebar.controlTextDidChange(Notification(name: NSControl.textDidChangeNotification, object: search))
        sidebar.resetFilter()
        XCTAssertEqual(search.stringValue, "")
        wait(for: [filtered], timeout: 0.3)
    }
    func testReplacingASelectedFolderDoesNotLeaveItsChildUnreachable() {
        _ = NSApplication.shared
        let sidebar = NavigationSidebar(frame: NSRect(x: 0, y: 0, width: 200, height: 500))
        func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
        let table = descendants(sidebar).compactMap { $0 as? NSTableView }.first { $0.accessibilityLabel() == "Workspace items" }!
        var opened = [String]()
        sidebar.onOpen = { opened.append($0.id) }
        sidebar.show([SidebarEntry(id: "folder", title: "Folder", detail: "", symbol: "folder", value: [:])], location: "Workspace", message: "")
        table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        sidebar.show([SidebarEntry(id: "folder/only.txt", title: "only.txt", detail: "", symbol: "doc", value: [:])], location: "Folder", message: "")
        XCTAssertEqual(table.selectedRow, -1)
        table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
        XCTAssertEqual(opened, ["folder", "folder/only.txt"])
        sidebar.show([], location: "No matches", message: "No matching files")
        XCTAssertEqual(table.selectedRow, -1)
    }

    func testLeftClickSidebarMenusAndRowOptionsDispatch() throws {
        _ = NSApplication.shared
        let nav = NavigationSidebar(frame: NSRect(x: 0, y: 0, width: 250, height: 500))
        func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
        var wsMenuCount = 0
        var itemMenuIds = [String]()
        nav.onWorkspaceMenu = { _ in wsMenuCount += 1 }
        nav.onItemMenu = { entry, _ in itemMenuIds.append(entry.id) }
        nav.show([SidebarEntry(id: "src/main.rs", title: "main.rs", detail: "", symbol: "doc", value: [:])], location: "Project", message: "")
        nav.layoutSubtreeIfNeeded()
        let table = descendants(nav).compactMap { $0 as? NSTableView }.first { $0.accessibilityLabel() == "Workspace items" }!
        _ = table.view(atColumn: 0, row: 0, makeIfNecessary: true)

        let buttons = descendants(nav).compactMap { $0 as? NSButton }
        let wsBtn = try XCTUnwrap(buttons.first { $0.accessibilityLabel() == "Sidebar workspace options" })
        wsBtn.performClick(nil)
        XCTAssertEqual(wsMenuCount, 1)

        let rowBtn = try XCTUnwrap(buttons.first { $0.accessibilityLabel() == "Options for main.rs" })
        rowBtn.performClick(nil)
        XCTAssertEqual(itemMenuIds, ["src/main.rs"])

        let conv = ConversationSidebar(folder: URL(fileURLWithPath: "/tmp/ws"))
        conv.frame = NSRect(x: 0, y: 0, width: 250, height: 500)
        var convWsMenu = 0
        var convItemIds = [String]()
        conv.onWorkspaceMenu = { _ in convWsMenu += 1 }
        conv.onItemMenu = { summary, _ in convItemIds.append(summary.id) }
        conv.show([ConversationSummary(id: "c1", title: "Main Session", detail: "Active")], selected: "c1", canSwitch: true, more: false)
        conv.layoutSubtreeIfNeeded()
        let convTable = descendants(conv).compactMap { $0 as? NSTableView }.first!
        _ = convTable.view(atColumn: 0, row: 0, makeIfNecessary: true)
        let convButtons = descendants(conv).compactMap { $0 as? NSButton }
        let convWsBtn = try XCTUnwrap(convButtons.first { $0.title == "Workspace Menu…" })
        convWsBtn.performClick(nil)
        XCTAssertEqual(convWsMenu, 1)
        let convRowBtn = try XCTUnwrap(convButtons.first { $0.accessibilityLabel() == "Options for Main Session" })
        convRowBtn.performClick(nil)
        XCTAssertEqual(convItemIds, ["c1"])

        // Verify right-click contextMenuForRow parity on both sidebars
        nav.buildItemMenu = { entry in NSMenu(title: entry.title) }
        conv.buildItemMenu = { summary in NSMenu(title: summary.title) }
        let navMenu = (table as? KeyboardListView)?.contextMenuForRow?(0)
        XCTAssertEqual(navMenu?.title, "main.rs")
        let convMenu = (convTable as? KeyboardListView)?.contextMenuForRow?(0)
        XCTAssertEqual(convMenu?.title, "Main Session")
    }
}
