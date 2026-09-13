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
            "A ribbon bends around the hill and returns to the sea.",
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
        assert_eq!(
            baselines.len(),
            json["data"]["lines"].as_array().unwrap().len()
        );
        assert_ne!(baselines[0][0][1], baselines[0][1][1]);
        assert!(json["data"]["glyphs"][0]["frame"].is_object());
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
