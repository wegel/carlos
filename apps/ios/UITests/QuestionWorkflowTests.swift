// Exercise native structured replies and retain private edits across sheet lifetimes.
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func testBrowserVisitDoesNotGrantConsent() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone browser")
        app.buttons["composer.send"].tap()
        let open = app.buttons["approval.browser"]
        XCTAssertTrue(open.waitForExistence(timeout: 15))
        let stop = app.buttons["composer.stop"]
        XCTAssertTrue(stop.exists)
        XCTAssertFalse(stop.isEnabled, "No native turn exists for this browser consent")
        capture("Browser consent names the destination and keeps the choice explicit", screenshot: app.screenshot())
        open.tap()
        let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")
        XCTAssertTrue(wait { safari.state == .runningForeground })
        XCTAssertTrue(safari.staticTexts["Browser step complete"].waitForExistence(timeout: 15))
        capture("Owned browser page completes without sending consent", screenshot: safari.screenshot())
        app.activate()
        XCTAssertTrue(open.waitForExistence(timeout: 15), "A page visit must leave consent unanswered")
        let allow = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'approval.' AND label == 'Allow once'")).firstMatch
        XCTAssertTrue(wait { allow.isEnabled }, "The current unchanged approval must be usable after returning from Safari")
        capture("Browser consent remains explicit and usable after reconnect", screenshot: app.screenshot())
        allow.tap()
        XCTAssertTrue(app.staticTexts["Browser consent accepted 🦀"].waitForExistence(timeout: 15))
        XCTAssertFalse(open.exists)
        app.terminate()
    }

    @MainActor
    func testNativeFormPreservesRejectedFieldsAndTypedValues() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone form")
        app.buttons["composer.send"].tap()
        let review = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'questions.open.'")).firstMatch
        XCTAssertTrue(review.waitForExistence(timeout: 15))
        review.tap()
        let count = app.textFields["question.count.text"]
        XCTAssertTrue(count.waitForExistence(timeout: 15))
        XCTAssertTrue(count.isEnabled)
        count.tap()
        count.typeText("10")
        if app.keyboards.firstMatch.exists {
            XCTAssertLessThanOrEqual(app.buttons["questions.submit"].frame.maxY, app.keyboards.firstMatch.frame.minY)
            let done = app.buttons["questions.keyboardDone"]
            XCTAssertTrue(done.waitForExistence(timeout: 5))
            XCTAssertLessThanOrEqual(done.frame.maxY, app.buttons["questions.submit"].frame.minY,
                                     "Done must stay in the top bar, clear of Submit, Deny and Cancel")
        }
        capture("Native form keeps Submit above the numeric keyboard", screenshot: app.screenshot())
        if app.buttons["questions.keyboardDone"].exists { app.buttons["questions.keyboardDone"].tap() }
        app.buttons["question.enabled.option.1"].tap()
        app.buttons["question.mode.option.1"].tap()
        let submit = app.buttons["questions.submit"]
        XCTAssertTrue(wait { submit.isEnabled })
        submit.tap()
        let error = app.staticTexts["questions.error"]
        XCTAssertTrue(error.waitForExistence(timeout: 15))
        XCTAssertTrue(error.label.contains("count"), "The backend must validate its original field constraint")
        capture("Rejected native form keeps entered values and explains the field error", screenshot: app.screenshot())
        XCTAssertEqual(count.value as? String, "10", "A rejected answer must keep private edits")
        XCTAssertEqual(app.buttons["question.mode.option.1"].value as? String, "Selected")
        XCTAssertTrue(wait { submit.isEnabled })
        count.tap()
        count.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 2) + "5")
        if app.buttons["questions.keyboardDone"].exists { app.buttons["questions.keyboardDone"].tap() }
        capture("Native form retains choices while correcting an invalid integer", screenshot: app.screenshot())
        submit.tap()
        XCTAssertTrue(app.staticTexts["Phone form accepted 🦀"].waitForExistence(timeout: 15))
        XCTAssertFalse(submit.exists)
        XCTAssertFalse(error.exists)
        app.terminate()
    }

    @MainActor
    func testNativeQuestionKeepsTheChoiceAndPrivateNote() throws {
        let app = try connectedSession()
        let draft = app.textViews["composer.draft"]
        XCTAssertTrue(draft.waitForExistence(timeout: 15))
        draft.tap()
        draft.typeText("phone questions")
        app.buttons["composer.send"].tap()
        let review = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'questions.open.'")).firstMatch
        XCTAssertTrue(review.waitForExistence(timeout: 15))
        review.tap()
        let option = app.buttons["question.field-str.option.0"]
        XCTAssertTrue(option.waitForExistence(timeout: 15))
        option.tap()
        XCTAssertEqual(option.value as? String, "Selected")
        let note = app.textFields["question.field-str.text"]
        XCTAssertTrue(note.waitForExistence(timeout: 15))
        note.tap()
        note.typeText("phone note 🦀")
        if app.buttons["questions.keyboardDone"].exists { app.buttons["questions.keyboardDone"].tap() }
        app.buttons["questions.close"].tap()
        review.tap()
        XCTAssertEqual(app.textFields["question.field-str.text"].value as? String, "phone note 🦀")
        capture("Native question with listed choice and private note", screenshot: app.screenshot())
        let submit = app.buttons["questions.submit"]
        XCTAssertTrue(wait { submit.isEnabled })
        submit.tap()
        XCTAssertTrue(app.staticTexts["Phone answers accepted 🦀"].waitForExistence(timeout: 15))
        XCTAssertFalse(submit.exists, "Only the observed native resolution removes the question")
        app.terminate()
    }

}
