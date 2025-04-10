use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::Arc;
// Removed sxd_document and sxd_xpath imports
use async_executors::AsyncStd;
use async_nursery::{NurseExt, Nursery};
use futures::StreamExt;
// Removed html5ever and markup5ever_rcdom imports
// Removed unused skyscraper::html import
use skyscraper::xpath; // Simplified xpath import

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

async fn process_input(
    input: InputJson,
) -> Result<HashMap<String, XpathResult>, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let input = Arc::new(input);
    let mut output_results = HashMap::new();
    // Removed xpath_factory

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
                // Removed factory_clone

                nursery
                    .nurse(async move {
                        let task_result: Result<bool, String> = (|| {
                            // Parse XPath using skyscraper
                            let xpath = xpath::parse(&xpath_str_clone) // Use simplified import
                                    .map_err(|e| format!("XPath parsing failed: {}", e))?;

                            let url_data = input_arc_clone
                                .urls
                                .get(&url_string_clone)
                                .ok_or_else(|| "Internal error: URL data not found".to_string())?;

                            let content_clone = url_data.content.clone();

                            // Check if target exists. If not, it's an automatic non-match.
                            let maybe_expected_target = url_data.targets.get(&heading_clone);
                            if maybe_expected_target.is_none() {
                                // No target specified, consider it a non-match for this URL/XPath pair
                                return Ok(false);
                            }
                            let expected_target = maybe_expected_target.unwrap(); // Safe to unwrap here

                            // Parse HTML using skyscraper
                            let document = skyscraper::html::parse(&content_clone)
                                .map_err(|e| format!("HTML parsing failed: {}", e))?;

                            // Create an item tree for XPath evaluation
                            let xpath_item_tree = xpath::XpathItemTree::from(&document); // Use simplified import

                            // Apply the XPath expression
                            let item_set = xpath
                                .apply(&xpath_item_tree)
                                .map_err(|e| format!("XPath evaluation failed: {}", e))?;

                            // Extract text content from the result (assuming we want the first node's text)
                            let actual_value: String = if item_set.is_empty() {
                                // Explicitly type actual_value
                                "".to_string() // No match found
                            } else {
                                // Attempt to get text from the first item in the set
                                // Trusting compiler error: assuming extract_as_tree_node returns &XpathItemTreeNode
                                item_set[0]
                                    .extract_as_node() // Assuming &Node<'_> based on prior errors/attempts
                                    .extract_as_tree_node() // Assuming &XpathItemTreeNode<'_> based on current error E0599
                                    .text(&xpath_item_tree) // Returns Option<String>
                                    .unwrap_or_default() // Returns String
                            };

                            // Compare with the expected target
                            let is_match = actual_value == *expected_target;
                            Ok(is_match)
                        })();

                        (url_string_clone, task_result)
                    })
                    .expect("Failed to spawn task");
            }

            drop(nursery);

            // The stream yields the task's return value directly: (String, Result<bool, String>)
            while let Some((url, comparison_result)) = output_stream.next().await {
                match comparison_result {
                    Ok(true) => successful_urls.push(url),
                    Ok(false) => unsuccessful_urls.push(url),
                    Err(e) => {
                        eprintln!(
                            "Error processing URL '{}' for XPath '{}': {}",
                            url, xpath_str, e
                        );
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
        let input_schema_value: serde_json::Value =
            serde_json::from_str(&input_schema).expect("Failed to parse input schema");
        let output_schema_value: serde_json::Value =
            serde_json::from_str(&output_schema).expect("Failed to parse output schema");

        // Compile schemas
        let input_compiled =
            JSONSchema::compile(&input_schema_value).expect("Failed to compile input schema");
        let output_compiled =
            JSONSchema::compile(&output_schema_value).expect("Failed to compile output schema");

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
        let input_value: serde_json::Value =
            serde_json::from_str(valid_input).expect("Failed to parse test input");
        assert!(input_compiled.is_valid(&input_value));

        // Test with invalid input (missing required fields)
        let invalid_input = r#"{"xpaths": {}}"#;
        let invalid_value: serde_json::Value =
            serde_json::from_str(invalid_input).expect("Failed to parse invalid input");
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
        let output_value: serde_json::Value =
            serde_json::from_str(valid_output).expect("Failed to parse test output");
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

    #[async_std::test]
    async fn test_parse_and_process_test_json() {
        // 1. Read the test.json file
        let json_content = fs::read_to_string("./test.json")
            .expect("Failed to read ./test.json. Make sure the file exists in the project root.");

        // 2. Parse the JSON content into InputJson
        let input: InputJson = serde_json::from_str(&json_content)
            .expect("Failed to parse ./test.json into InputJson struct.");

        // 3. Process the input
        let result = process_input(input)
            .await
            .expect("process_input failed when running with content from ./test.json");

        // 4. Print the result for inspection
        println!("--- Output from process_input with test.json ---");
        dbg!(&result);
        println!("------------------------------------------------");

        // 5. Deliberately fail the test to show the output
        panic!("Deliberately failing test_parse_and_process_test_json to show output.");
    }

    #[test]
    fn test_parse_html_with_special_chars() {
        // Test parsing HTML with escaped quotes in attributes and special characters in text content
        //        let html_fragment = r#"<js-searchapivalidator computed="false" kind="init" method="false" shorthand="false"> /[.*?#%^$&!<>,:;'=@{}()|[\\]\\\\]/g </js-searchapivalidator>"#;

        let html_fragment = r#"<js-searchapivalidator computed="false" kind="init" method="false" shorthand="false">/[.*?#%^$&!,:;'=@{}()|[\\]\\\\]/g</js-searchapivalidator>"#;

        // Attempt to parse the fragment
        let parse_result = skyscraper::html::parse(html_fragment);

        // Assert that parsing was successful (did not return Err)
        assert!(
            parse_result.is_ok(),
            "Failed to parse HTML fragment with special characters: {:?}",
            parse_result.err()
        );
    }

    #[test]
    fn test_parse_html_with_escaped_chars_and_custom_tags() {
        // Test parsing HTML with escaped characters (&amp;, &lt;, &gt;, &#39;) and custom tags
        let html_fragment = r#"<js-Program sourceType="script"><js-body><js-VariableDeclaration kind="const"><js-declarations><js-VariableDeclarator><js-id>x</js-id><js-init><js-ObjectExpression><js-properties><js-searchApiValidator computed="false" kind="init" method="false" shorthand="false">/[.*?#%^$&amp;!&lt;&gt;,:;&#39;=@{}()|[\\]\\\\]/g</js-searchApiValidator></js-properties></js-ObjectExpression></js-init></js-VariableDeclarator></js-declarations></js-VariableDeclaration></js-body></js-Program>"#;

        // Attempt to parse the fragment
        let parse_result = skyscraper::html::parse(html_fragment);

        // Assert that parsing was successful (did not return Err)
        assert!(
            parse_result.is_ok(),
            "Failed to parse HTML fragment with escaped chars and custom tags: {:?}",
            parse_result.err()
        );
    }
}
