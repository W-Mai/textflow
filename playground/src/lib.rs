#[cfg(any(target_arch = "wasm32", test))]
mod archive;
mod font;
mod scaffold;
mod scenes;

pub use scaffold::{catalog, files};
pub use scenes::SCENES;
pub use scenes::{analyze, Options, Response};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::{analyze, archive, catalog, files, Options, SCENES};
    use serde::Serialize;
    use wasm_bindgen::prelude::*;

    fn object<T: Serialize>(value: &T) -> JsValue {
        let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
        value.serialize(&serializer).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen]
    pub fn analyze_scene(scene: &str, text: &str, options: JsValue) -> JsValue {
        let options = serde_wasm_bindgen::from_value::<Options>(options);
        match options {
            Ok(options) => object(&analyze(scene, text, &options)),
            Err(error) => object(&serde_json::json!({
                "scene": scene,
                "ok": false,
                "error": { "kind": "InvalidOptions", "message": error.to_string() }
            })),
        }
    }

    #[wasm_bindgen]
    pub fn feature_catalog() -> JsValue {
        object(&catalog())
    }

    #[wasm_bindgen]
    pub fn scene_catalog() -> JsValue {
        object(&SCENES)
    }

    #[wasm_bindgen]
    pub fn scaffold_files(selected: Vec<String>) -> JsValue {
        match files(&selected) {
            Ok(files) => object(&serde_json::json!({ "ok": true, "files": files })),
            Err(error) => object(&serde_json::json!({ "ok": false, "error": error })),
        }
    }

    #[wasm_bindgen]
    pub fn zip_files(files: JsValue) -> Result<Vec<u8>, JsValue> {
        let files: Vec<archive::ArchiveFile> = serde_wasm_bindgen::from_value(files)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        archive::zip(&files).map_err(|error| JsValue::from_str(&error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_uses_real_line_breaks() {
        let result = analyze(
            "core",
            "A small UI can still set type well.",
            &Options {
                width: 192,
                ..Options::default()
            },
        );
        assert!(result.ok);
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["data"]["lines"][0]["text"], "A small UI");
    }

    #[test]
    fn core_width_control_reaches_cjk_breaks() {
        let result = analyze(
            "core",
            "文本排版",
            &Options {
                width: 32,
                ..Options::default()
            },
        );
        let json = serde_json::to_value(result).unwrap();
        assert_eq!(json["data"]["lines"].as_array().unwrap().len(), 4);
        assert_eq!(json["data"]["lines"][0]["text"], "文");
        assert_eq!(json["code"], "TextFlow::new(text, 2)");
    }

    #[test]
    fn geometry_exposes_the_baselines_used_for_placement() {
        let result = analyze(
            "geometry",
            "Lorem ipsum dolor sit amet.",
            &Options {
                width: 32,
                ..Options::default()
            },
        );
        assert!(
            result.ok,
            "{:?}",
            result.error.as_ref().map(|error| &error.message)
        );
        let json = serde_json::to_value(result).unwrap();
        let baselines = json["data"]["baselines"].as_array().unwrap();
        assert_eq!(baselines.len(), 1);
        assert!(baselines[0].as_array().unwrap().len() > 2);
        assert_ne!(baselines[0][0][1], baselines[0][1][1]);
        assert!(json["data"]["glyphs"][0]["frame"].is_object());
        assert!(json["data"]["carets"][0]["frame"].is_object());
    }

    #[test]
    fn geometry_places_on_a_custom_curve() {
        let result = analyze(
            "geometry",
            "Lorem",
            &Options {
                path: Some(vec![[0, 96], [80, 64], [176, 116], [288, 84]]),
                ..Options::default()
            },
        );
        assert!(result.ok, "{:?}", result.error.as_ref().map(|e| &e.message));
        let json = serde_json::to_value(result).unwrap();
        let path = json["data"]["baselines"][0].as_array().unwrap();
        assert_eq!(path.first().unwrap(), &serde_json::json!([0, 3000]));
        assert_eq!(path.last().unwrap(), &serde_json::json!([9000, 2625]));
        assert!(json["data"]["glyphs"][0]["frame"].is_object());
    }

    #[test]
    fn changing_font_size_keeps_the_curve_in_path_space() {
        let path = Some(vec![[0, 96], [80, 64], [176, 116], [288, 84]]);
        let mut projected = Vec::new();
        for font_size in [24, 48] {
            let result = analyze(
                "geometry",
                "Lorem",
                &Options {
                    font_size,
                    path: path.clone(),
                    ..Options::default()
                },
            );
            assert!(result.ok);
            let json = serde_json::to_value(result).unwrap();
            let points = json["data"]["baselines"][0].as_array().unwrap();
            projected.push(
                points
                    .iter()
                    .map(|point| {
                        point
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|value| value.as_i64().unwrap() * i64::from(font_size) / 1000)
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>(),
            );
        }
        for (left, right) in projected[0].iter().zip(&projected[1]) {
            assert!((left[0] - right[0]).abs() <= 1);
            assert!((left[1] - right[1]).abs() <= 1);
        }
    }

    #[test]
    fn default_curve_holds_the_sample_at_every_font_size() {
        let sample = SCENES
            .iter()
            .find(|scene| scene.id == "geometry")
            .unwrap()
            .sample;
        for font_size in [16, 32, 48, 64] {
            let result = analyze(
                "geometry",
                sample,
                &Options {
                    font_size,
                    ..Options::default()
                },
            );
            assert!(
                result.ok,
                "{font_size}: {:?}",
                result.error.map(|error| error.message)
            );
        }
    }

    #[test]
    fn geometry_rejects_short_and_unbounded_paths() {
        let long_text = analyze("geometry", &"a".repeat(81), &Options::default());
        assert_eq!(long_text.error.unwrap().kind, "TextLimit");
        let short = analyze(
            "geometry",
            "Lorem ipsum dolor sit amet.",
            &Options {
                path: Some(vec![[0, 64], [32, 64]]),
                ..Options::default()
            },
        );
        assert_eq!(short.error.unwrap().kind, "PathTooShort");
        let dense = analyze(
            "geometry",
            "Lorem",
            &Options {
                path: Some(vec![[0, 0]; 65]),
                ..Options::default()
            },
        );
        assert_eq!(dense.error.unwrap().kind, "InvalidPath");
        let extreme = analyze(
            "geometry",
            "Lorem",
            &Options {
                path: Some(vec![[i32::MIN, 0], [0, 0]]),
                ..Options::default()
            },
        );
        assert_eq!(extreme.error.unwrap().kind, "InvalidPath");
    }

    #[test]
    fn all_scenes_return_typed_results_or_explicit_errors() {
        let samples = [
            ("unicode", "你好 e\u{301} hello"),
            ("bidi", "abc مرحبا"),
            ("shaping", "A small UI"),
            ("arabic", "باب"),
            ("thai", "ภาษาไทย"),
            ("devanagari", "कमल"),
            ("workspace", "A small UI"),
            ("geometry", "Lines follow borrowed widths"),
        ];
        for (scene, text) in samples {
            let result = analyze(scene, text, &Options::default());
            assert!(
                result.ok,
                "{scene}: {:?}",
                result.error.as_ref().map(|error| &error.message)
            );
        }
    }

    #[test]
    fn catalog_samples_use_real_scene_dispatch() {
        for scene in SCENES {
            let result = analyze(scene.id, scene.sample, &Options::default());
            assert!(
                result.ok,
                "{}: {:?}",
                scene.id,
                result.error.as_ref().map(|error| &error.message)
            );
        }
    }

    #[test]
    fn workspace_limit_is_visible() {
        let result = analyze(
            "workspace",
            "a long input",
            &Options {
                text_limit: 2,
                ..Options::default()
            },
        );
        assert!(!result.ok);
        assert_eq!(result.error.unwrap().kind, "Workspace");
    }
}
