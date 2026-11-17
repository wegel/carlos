#!/usr/bin/env bash
# Build and test only Carlos sources, libraries, app and simulator on the owned Mac.
set -euo pipefail

SOURCE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="$(cd "$SOURCE/../.." && pwd)"
BUILD="$BASE/builds/$(basename "$SOURCE")"
export PATH="$BASE/toolchains/rust-1.100.0/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export CARGO_HOME="$BASE/toolchains/cargo"
export CARGO_TARGET_DIR="$BASE/rust-target"
export CARLOS_GENERATED="$BUILD/generated"
export CARLOS_APP="$SOURCE/apps/ios"
DEPENDENCIES="$(cat "$SOURCE/dependencies-id")"
VENDOR="$BASE/ios-dependencies/$DEPENDENCIES/vendor"
mkdir -p "$BUILD" "$CARLOS_GENERATED"
cd "$SOURCE"
APP_RUNNER=(/usr/bin/env)

framework() {
    rust_build --target x86_64-apple-ios
    rust_build --target aarch64-apple-ios
    rust_build --target aarch64-apple-ios-sim
    rust_build --features bindgen --bin carlos-bindgen-swift
    local library="$CARLOS_GENERATED/libcarlos_phone.a"
    lipo -create "$CARGO_TARGET_DIR/x86_64-apple-ios/release/libcarlos_phone.a" \
        "$CARGO_TARGET_DIR/aarch64-apple-ios-sim/release/libcarlos_phone.a" -output "$library"
    "$CARGO_TARGET_DIR/release/carlos-bindgen-swift" "$library" "$CARLOS_GENERATED/Swift" --swift-sources
    "$CARGO_TARGET_DIR/release/carlos-bindgen-swift" "$library" "$CARLOS_GENERATED/Headers" \
        --headers --modulemap --module-name carlos_phoneFFI --modulemap-filename module.modulemap
    # Each immutable source bundle has its own product directory; do not replace checked frameworks.
    if [[ ! -d "$CARLOS_GENERATED/CarlosPhone.xcframework" ]]; then
        xcodebuild -create-xcframework \
            -library "$library" -headers "$CARLOS_GENERATED/Headers" \
            -library "$CARGO_TARGET_DIR/aarch64-apple-ios/release/libcarlos_phone.a" \
            -headers "$CARLOS_GENERATED/Headers" -output "$CARLOS_GENERATED/CarlosPhone.xcframework"
    fi
    # Keep the spec and project together: framework references are relative to the spec.
    cp "$SOURCE/apps/ios/project.yml" "$BUILD/project.yml"
    xcodegen generate --spec "$BUILD/project.yml" --project "$BUILD"
}

rust_build() {
    # An absolute shared vendor path keeps Cargo fingerprints stable between source bundles.
    cargo build --offline --locked --release -p carlos-phone \
        --config "source.vendored-sources.directory=\"$VENDOR\"" "$@"
}

app() {
    "${APP_RUNNER[@]}" xcodebuild -quiet -project "$BUILD/Carlos.xcodeproj" -scheme Carlos \
        -configuration "${CARLOS_BUILD_CONFIGURATION:-Debug}" -sdk iphonesimulator -derivedDataPath "$BUILD/DerivedData" \
        CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=- "$@"
}

case "${1:-}" in
framework) framework ;;
build) app build -destination 'generic/platform=iOS Simulator' ;;
test)
    : "${2:?pass the owned Carlos simulator UUID}"
    app test -destination "platform=iOS Simulator,id=$2" \
        -only-testing:CarlosTests \
        -parallel-testing-enabled NO -maximum-concurrent-test-simulator-destinations 1 \
        -resultBundlePath "$BUILD/tests-$(date +%Y%m%dT%H%M%S).xcresult"
    ;;
ui-test)
    : "${2:?pass the owned Carlos simulator UUID}" "${3:?pass the owned fixture path}"
    xcrun simctl bootstatus "$2" -b
    mkdir -p "$CARLOS_GENERATED/UIFixtures"
    cp "$BASE/$3/fixture.json" "$CARLOS_GENERATED/UIFixtures/fixture.json"
    cp "$SOURCE/apps/ios/project.yml" "$BUILD/project.yml"
    xcodegen generate --spec "$BUILD/project.yml" --project "$BUILD"
    workflow="$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("workflow", "prompt"))' "$BASE/$3/fixture.json")"
    case "$workflow" in
        prompt) test_case=testConnectCreateAndSendOneExactPrompt ;;
        quota) test_case=testClaudeWeeklyQuotaSitsBesideContext ;;
        claude-tools) test_case=testClaudeCallAndResultShareOneCompactHeader ;;
        controls) test_case=testApprovalAndStopPassAHeldPromptReply ;;
        source) test_case=testLongOriginalSourceAndCopyKeepEveryByte ;;
        code-source) test_case=testLargeCodeFenceKeepsEveryOriginalByteAndStaysResponsive ;;
        reference) test_case=testReadOnlyReferenceKeepsFixedControlsInBothLayouts ;;
        relaunch) test_case=testRelaunchReusesSavedConnectionAndToken ;;
        credentials) test_case=testInvalidTokenKeepsTheFormAndAllowsCorrection ;;
        token-update) test_case=testRejectedReplacementKeepsTheSavedTokenThroughCancelAndRestart ;;
        tools) test_case=testRunningToolExpandsStreamsAndKeepsItsRowChoice ;;
        questions) test_case=testNativeQuestionKeepsTheChoiceAndPrivateNote ;;
        form) test_case=testNativeFormPreservesRejectedFieldsAndTypedValues ;;
        browser) test_case=testBrowserVisitDoesNotGrantConsent ;;
        scroll) test_case=testToolExpansionAndNativeUpdatesKeepTheReadersPlace ;;
        history) test_case=testReachingTheTopLoadsOlderHistoryWithoutMovingText ;;
        ralph) test_case=testRalphUsesEngineFactsAndKeepsStopAvailable ;;
        syntax) test_case=testCodeColorsKeepTheOriginalMarkdownOnTheClipboard ;;
        idle-perf) test_case=testIdleCpuAndResidentMemory ;;
        launch-perf) test_case=testSavedLaunchPerformance ;;
        typing-perf) test_case=testPrivateTypingAndNativeScrollPerformance ;;
        scroll-perf) test_case=testNativeScrollPerformance ;;
        reconnect) test_case=testRestartRetainsPrivateTextWithoutResendingAndReopensTheSession ;;
        voice)
            # Reset only this synthetic app so the real native permission prompt repeats.
            xcrun simctl privacy "$2" reset all app.carlos.phone.fixture
            test_case=testUnavailableOrDeniedVoiceInputKeepsThePrivateDraft
            ;;
        keyboard) test_case=testHardwareReturnSendsAndModifiedReturnKeepsNewlines ;;
        code-stream) test_case=testStreamingCaretBlinksAndStopsWithTheOriginalTurn ;;
        stream) test_case=testStreamingCaretBlinksAndStopsWithTheOriginalTurn ;;
        *) echo "unknown phone fixture workflow" >&2; exit 2 ;;
    esac
    checks=(-only-testing:"CarlosUITests/SessionWorkflowTests/$test_case")
    if [[ "$workflow" == keyboard ]]; then
        checks+=(-only-testing:CarlosTests/PromptInputTests)
        xcrun clang -fobjc-arc -fblocks -framework Foundation "$SOURCE/ios-keyboard.m" -o "$BUILD/send-key"
        APP_RUNNER=(/usr/bin/python3 "$SOURCE/ios-keyboard-test.py" "$BUILD/send-key" "$2" "$CARLOS_GENERATED/UIFixtures/fixture.json")
    fi
    if [[ "$workflow" == idle-perf ]]; then
        APP_RUNNER=(/usr/bin/python3 "$SOURCE/ios-idle-test.py" "$2" "$CARLOS_GENERATED/UIFixtures/fixture.json" "$BUILD/idle-metrics.json")
    fi
    CARLOS_BUILD_CONFIGURATION=Fixture app test -destination "platform=iOS Simulator,id=$2" \
        "${checks[@]}" -parallel-testing-enabled NO \
        -test-timeouts-enabled YES -default-test-execution-time-allowance 180 \
        -maximum-test-execution-time-allowance 180 \
        -maximum-concurrent-test-simulator-destinations 1 \
        -resultBundlePath "$BUILD/ui-tests-$(date +%Y%m%dT%H%M%S).xcresult"
    ;;
launch)
    : "${2:?pass the owned Carlos simulator UUID}"
    xcrun simctl bootstatus "$2" -b
    xcrun simctl install "$2" "$BUILD/DerivedData/Build/Products/Debug-iphonesimulator/Carlos.app"
    xcrun simctl launch "$2" app.carlos.phone
    xcrun simctl io "$2" screenshot "$BUILD/screen.png"
    ;;
*) echo "usage: ios-remote.sh framework|build|test <simulator>|launch <simulator>" >&2; exit 2 ;;
esac
