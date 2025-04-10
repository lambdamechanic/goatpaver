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
    let xpath_factory = Factory::new();

    for (heading, xpath_list) in &input.xpaths {
        for xpath_str in xpath_list {
            let mut successful_urls = Vec::new();
            let mut unsuccessful_urls = Vec::new();

            let compiled_xpath_result: Result<Arc<XPath>, _> = xpath_factory.build(xpath_str).map(Arc::new);

            match compiled_xpath_result {
                Ok(compiled_xpath_arc) => {
                    let (nursery, mut output_stream) = Nursery::new(AsyncStd);

                    for url_string in input.urls.keys() {
                        let input_arc_clone = Arc::clone(&input);
                        let compiled_xpath_arc_clone = Arc::clone(&compiled_xpath_arc);
                        let url_string_clone = url_string.clone();
                        let heading_clone = heading.clone();

                        nursery.nurse(async move {
                            let task_result: Result<bool, String> = (|| {
                                let url_data = input_arc_clone.urls.get(&url_string_clone)
                                    .ok_or_else(|| "Internal error: URL data not found".to_string())?;

                                let content_clone = url_data.content.clone();
                                let expected_target = url_data.targets.get(&heading_clone)
                                    .ok_or_else(|| "No target specified".to_string())?;

                                let package = parser::parse(&content_clone)
                                    .map_err(|e| format!("HTML parsing failed: {}", e))?;
                                let document = package.as_document();
                                let context = Context::new();

                                let eval_result = compiled_xpath_arc_clone.evaluate(&context, document.root())
                                    .map_err(|e| format!("XPath evaluation failed: {}", e))?;

                                let is_match = match eval_result {
                                    Value::String(actual_value) => actual_value == *expected_target,
                                    Value::Nodeset(nodeset) => {
                                        let actual_value = if nodeset.is_empty() {
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

                    while let Some(task_join_result) = output_stream.next().await {
                        match task_join_result {
                            Ok((url, comparison_result)) => {
                                match comparison_result {
                                    Ok(true) => successful_urls.push(url),
                                    Ok(false) => unsuccessful_urls.push(url),
                                    Err(e) => {
                                        eprintln!("Error processing URL '{}' for XPath '{}': {}", url, xpath_str, e);
                                        unsuccessful_urls.push(url);
                                    }
                                }
                            }
                            Err(join_error) => {
                                eprintln!("Task panicked or was cancelled for XPath '{}': {:?}", xpath_str, join_error);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("XPath compilation failed for '{}': {}", xpath_str, e);
                    for url_string in input.urls.keys() {
                        unsuccessful_urls.push(url_string.clone());
                    }
                }
            }

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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
