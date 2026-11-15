//! Preserve user-visible rewind positions without exposing hidden native reminder prompts.

use serde_json::json;

use super::{is_hidden_rewind_point, parse_rewind_points, resolve_rewind_target, GrokRewindPoint};

#[test]
fn parse_rewind_points_reads_snake_case_list() {
    let points = parse_rewind_points(&json!({
        "rewind_points": [
            {
                "prompt_index": 0,
                "prompt_preview": "first"
            },
            {
                "prompt_index": 1,
                "prompt_preview": "second"
            }
        ]
    }));
    assert_eq!(
        points,
        [
            GrokRewindPoint {
                prompt_index: 0,
                prompt_preview: "first".to_string()
            },
            GrokRewindPoint {
                prompt_index: 1,
                prompt_preview: "second".to_string()
            }
        ]
    );
}

#[test]
fn resolve_rewind_target_uses_user_facing_index() {
    let points = vec![
        GrokRewindPoint {
            prompt_index: 0,
            prompt_preview: "<system-reminder>hidden".to_string(),
        },
        GrokRewindPoint {
            prompt_index: 1,
            prompt_preview: "first".to_string(),
        },
        GrokRewindPoint {
            prompt_index: 2,
            prompt_preview: "second".to_string(),
        },
    ];
    assert!(is_hidden_rewind_point("<system-reminder>hidden"));
    assert_eq!(resolve_rewind_target(&points, 1).expect("target"), 2);
}
