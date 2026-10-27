"""Fixed RTK 0.45-style output; terminal checks do not need RTK installed."""

READ_COMMAND = "rtk --verbose proxy --skip-env -- rtk read -n 'main file.rs'"
TEST_COMMAND = "rtk --ultra-compact cargo test -p demo"
CHAIN_COMMAND = "rtk proxy git status && rtk proxy git diff --stat"
SEARCH_COMMAND = "rtk proxy rg -n 'withPrivateMovieSource|catalogServiceURL|seriesHierarchyKey' apps/ios/Cinero/VendorEnrollmentProofTests/Catalog* apps/ios/Cinero/Cinero/Services/CatalogStore* | head -n 55"


def search_items():
    return [{"type": "commandExecution", "id": "ep130-search", "command": SEARCH_COMMAND,
             "aggregatedOutput": "CatalogStore.swift:55:catalogServiceURL", "exitCode": 0}]


def rtk_items(failure=False):
    if failure:
        return [{"type": "commandExecution", "id": "rtk-failure",
                 "command": "rtk --verbose proxy head -n 20 missing.rs",
                 "aggregatedOutput": "head: missing.rs: no such file\n[full output: /tmp/rtk-read.log]",
                 "exitCode": 1}]
    return [{"type": "commandExecution", "id": "rtk-read", "command": READ_COMMAND,
             "commandActions": [{"type": "unknown", "command": "rtk read -n 'main file.rs'"}],
             "aggregatedOutput": '1 │ fn main() {\n2 │ let value = "rtk hello";\n3 │ }\n', "exitCode": 0},
            {"type": "commandExecution", "id": "rtk-test", "command": TEST_COMMAND,
             "commandActions": [{"type": "unknown", "command": "cargo test -p demo"}],
             "aggregatedOutput": "cargo test: 3 passed", "exitCode": 0},
            {"type": "commandExecution", "id": "rtk-chain", "command": CHAIN_COMMAND,
             "commandActions": [{"type": "unknown", "command": "git status"},
                                {"type": "unknown", "command": "git diff --stat"}],
             "aggregatedOutput": "chain status marker\nchain diff marker", "exitCode": 0}]
