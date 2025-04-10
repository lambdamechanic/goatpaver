use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::Arc;
use sxd_document::parser;
use sxd_xpath::{Context, Factory, Value, XPath};
use async_nursery::{Nursery, NurseExt};
use async_executors::AsyncStd;
use futures::StreamExt;

// --- Input Structures ---

#[derive(Deserialize, Debug)]
struct InputJson {
    xpaths: HashMap<String, Vec<String>>,
    urls: HashMap<String, UrlData>,
}

#[derive(Deserialize, Debug)]
struct UrlData {
    // We don't need targets for the stub
    targets: HashMap<String, String>,
    content: String,
}

// --- Output Structures ---

#[derive(Serialize, Debug)]
struct XpathResult {
    successful: Vec<String>,
    unsuccessful: Vec<String>,
}

async fn process_input(input: InputJson) -> Result<HashMap<String, XpathResult>, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let input = Arc::new(input);
    let mut output_results = HashMap::new();
    let xpath_factory = Arc::new(Factory::new());

    for (heading, xpath_list) in &input.xpaths {
        for xpath_str in xpath_list {
            let mut successful_urls = Vec::new();
            let mut unsuccessful_urls = Vec::new();

            let (nursery, mut output_stream) = Nursery::new(AsyncStd);

            for url_string in input.urls.keys() {
                let input_arc_clone = Arc::clone(&input);
                let url_string_clone = url_string.clone();
                let heading_clone = heading.clone();
                let xpath_str_clone = xpath_str.clone();
                let factory_clone = Arc::clone(&xpath_factory);

                        nursery.nurse(async move {
                            let task_result: Result<bool, String> = (|| {
                                // Compile XPath inside the task
                                let compiled_xpath = factory_clone
                                    .build(&xpath_str_clone)
                                    .map_err(sxd_xpath::Error::from)
                                    .and_then(|maybe_xpath| maybe_xpath.ok_or(sxd_xpath::Error::NoXPath))
                                    .map_err(|e| format!("XPath compilation failed: {}", e))?;

                                let url_data = input_arc_clone.urls.get(&url_string_clone)
                                    .ok_or_else(|| "Internal error: URL data not found".to_string())?;

                                let content_clone = url_data.content.clone();
                                let expected_target = url_data.targets.get(&heading_clone)
                                    .ok_or_else(|| "No target specified".to_string())?;

                                let package = parser::parse(&content_clone)
                                    .map_err(|e| format!("HTML parsing failed: {}", e))?;
                                let document = package.as_document();
                                let context = Context::new();

                                let eval_result = compiled_xpath.evaluate(&context, document.root())
                                    .map_err(|e| format!("XPath evaluation failed: {}", e))?;

                                let is_match = match eval_result {
                                    Value::String(actual_value) => actual_value == *expected_target,
                                    Value::Nodeset(nodeset) => {
                                        let actual_value = if nodeset.size() == 0 {
                                            "".to_string()
                                        } else {
                                            nodeset.document_order_first().map_or("".to_string(), |n| n.string_value())
                                        };
                                        actual_value == *expected_target
                                    }
                                    _ => false,
                                };
                                Ok(is_match)
                            })();

                            (url_string_clone, task_result)
                        }).expect("Failed to spawn task");
                    }

                    drop(nursery);

                    // The stream yields the task's return value directly: (String, Result<bool, String>)
                    while let Some((url, comparison_result)) = output_stream.next().await {
                        match comparison_result {
                            Ok(true) => successful_urls.push(url),
                            Ok(false) => unsuccessful_urls.push(url),
                            Err(e) => {
                                eprintln!("Error processing URL '{}' for XPath '{}': {}", url, xpath_str, e);
                                unsuccessful_urls.push(url); // Add to unsuccessful if the inner task failed
                            }
                        }
                    } // Panics in spawned tasks are implicitly handled by nursery/executor (may panic main thread or be ignored)

            output_results
                .entry(xpath_str.clone())
                .or_insert_with(|| XpathResult {
                    successful: successful_urls,
                    unsuccessful: unsuccessful_urls,
                });
        }
    }

    Ok(output_results)
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    // 1. Read stdin
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    // 2. Deserialize input
    let input: InputJson = serde_json::from_str(&buffer)?;

    // --- Call the processing function ---
    let output: HashMap<String, XpathResult> = process_input(input).await?;
    // --- End call ---

    // 5. Serialize output
    let output_json_string = serde_json::to_string_pretty(&output)?; // Use pretty print for readability

    // 6. Print to stdout
    println!("{}", output_json_string);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*; // Import items from the parent module (main)
    use jsonschema::JSONSchema;
    use std::fs;

    #[async_std::test]
    async fn test_schema_validation() {
        // Load schemas
        let input_schema = fs::read_to_string("schemas/input.schema.json")
            .expect("Failed to read input schema file");
        let output_schema = fs::read_to_string("schemas/output.schema.json")
            .expect("Failed to read output schema file");

        // Parse schemas
        let input_schema_value: serde_json::Value = serde_json::from_str(&input_schema)
            .expect("Failed to parse input schema");
        let output_schema_value: serde_json::Value = serde_json::from_str(&output_schema)
            .expect("Failed to parse output schema");

        // Compile schemas
        let input_compiled = JSONSchema::compile(&input_schema_value)
            .expect("Failed to compile input schema");
        let output_compiled = JSONSchema::compile(&output_schema_value)
            .expect("Failed to compile output schema");

        // Test with valid input
        let valid_input = r#"
        {
            "xpaths": {
                "test": ["//test"]
            },
            "urls": {
                "http://example.com": {
                    "targets": {
                        "test": "value"
                    },
                    "content": "<html></html>"
                }
            }
        }
        "#;
        let input_value: serde_json::Value = serde_json::from_str(valid_input)
            .expect("Failed to parse test input");
        assert!(input_compiled.is_valid(&input_value));

        // Test with invalid input (missing required fields)
        let invalid_input = r#"{"xpaths": {}}"#;
        let invalid_value: serde_json::Value = serde_json::from_str(invalid_input)
            .expect("Failed to parse invalid input");
        assert!(!input_compiled.is_valid(&invalid_value));

        // Test output schema with valid output
        let valid_output = r#"
        {
            "//test": {
                "successful": ["http://example.com"],
                "unsuccessful": []
            }
        }
        "#;
        let output_value: serde_json::Value = serde_json::from_str(valid_output)
            .expect("Failed to parse test output");
        assert!(output_compiled.is_valid(&output_value));
    }

    #[async_std::test]
    async fn test_process_input_real_logic_expected_failure() {
        // 1. Prepare input data with HTML content
        let input_json_string = r#"
        {
            "xpaths": {
                "Content Selectors": ["/html/body/p"],
                "Link Selectors": ["//a[@id='link1']"],
                "Nonexistent Selectors": ["//div[@class='nonexistent']"]
            },
            "urls": {
                "http://site1.com": {
                    "targets": {
                        "Content Selectors": "Site 1 paragraph",
                        "Link Selectors": "Link 1"
                    },
                    "content": "<html><body><p>Site 1 paragraph</p><a id='link1'>Link 1</a></body></html>"
                },
                "http://site2.com": {
                    "targets": {
                        "Content Selectors": "Site 2 paragraph"

                    },
                    "content": "<html><body><p>Site 2 paragraph</p><b>No link here</b></body></html>"
                }
            }
        }
        "#;
        let input: InputJson =
            serde_json::from_str(input_json_string).expect("Failed to parse test input JSON");

        // 2. Call the function under test
        let output: HashMap<String, XpathResult> = process_input(input)
            .await
            .expect("Processing failed in test");

        // 3. Define expected output (based on real logic, not the stub)
        let mut expected_results = HashMap::new();

        // XPath: "/html/body/p" - Should match both
        let mut urls_p = vec![
            "http://site1.com".to_string(),
            "http://site2.com".to_string(),
        ];
        urls_p.sort();
        expected_results.insert(
            "/html/body/p".to_string(),
            XpathResult {
                successful: urls_p,
                unsuccessful: Vec::new(),
            },
        );

        // XPath: "//a[@id='link1']"
        let mut urls_a_succ = vec!["http://site1.com".to_string()];
        urls_a_succ.sort();
        let mut urls_a_unsucc = vec!["http://site2.com".to_string()];
        urls_a_unsucc.sort();
        expected_results.insert(
            "//a[@id='link1']".to_string(),
            XpathResult {
                successful: urls_a_succ,
                unsuccessful: urls_a_unsucc,
            },
        );

        // XPath: "//div[@class='nonexistent']"
        let mut urls_div_unsucc = vec![
            "http://site1.com".to_string(),
            "http://site2.com".to_string(),
        ];
        urls_div_unsucc.sort();
        expected_results.insert(
            "//div[@class='nonexistent']".to_string(),
            XpathResult {
                unsuccessful: urls_div_unsucc,
                successful: Vec::new(),
            },
        );

        // 4. Assertions (These are expected to fail with the current stub implementation)
        assert_eq!(
            output.len(),
            expected_results.len(),
            "Number of XPaths in output mismatch"
        );

        for (xpath, result) in output {
            let expected_result = expected_results
                .get(&xpath)
                .expect(&format!("Unexpected XPath key in output: {}", xpath));

            // Sort actual results for comparison
            let mut sorted_actual_successful = result.successful;
            sorted_actual_successful.sort();
            let mut sorted_actual_unsuccessful = result.unsuccessful;
            sorted_actual_unsuccessful.sort();

            // Compare sorted lists
            assert_eq!(
                sorted_actual_successful, expected_result.successful,
                "Successful URLs mismatch for XPath: {}",
                xpath
            );
            assert_eq!(
                sorted_actual_unsuccessful, expected_result.unsuccessful,
                "Unsuccessful URLs mismatch for XPath: {}",
                xpath
            );
        }
    }
}
