use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read};
use sxd_document::parser;
use sxd_xpath::{Context, Factory, Value};

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

fn process_input(input: InputJson) -> HashMap<String, XpathResult> {

    // Pre-parse all HTML documents
    let packages: HashMap<String, Result<sxd_document::Package, _>> = input
        .urls
        .iter()
        .map(|(url, url_data)| (url.clone(), parser::parse(&url_data.content)))
        .collect();

    let mut output_results = HashMap::new();
    let xpath_factory = Factory::new();

    // Iterate through headings and their associated XPath lists
    for (heading, xpath_list) in &input.xpaths {
        // Iterate through individual XPath strings in the list
        for xpath_str in xpath_list {
            let mut successful_urls = Vec::new();
            let mut unsuccessful_urls = Vec::new();

            // Attempt to compile the XPath expression once
            let xpath = match xpath_factory.build(xpath_str) {
                Ok(xp) => xp,
                Err(_) => {
                    // If XPath compilation fails, all URLs are unsuccessful for this XPath
                    None
                }
            };

            // Iterate through each URL to check this XPath
            for (url_string, url_data) in &input.urls {
                // Check if a target exists for this heading and URL
                if let Some(expected_target_str) = url_data.targets.get(heading) {
                    // Target exists, proceed with evaluation
                    let expected_target = expected_target_str.as_str(); // Convert &String to &str

                    // First, check if XPath compilation was successful
                    if let Some(compiled_xpath_ref) = xpath.as_ref() {
                        // Second, check if the document parsing was successful for this URL
                        match packages.get(url_string).unwrap() {
                            Ok(package) => {
                                // Both XPath and Document are valid, proceed with evaluation
                                let document = package.as_document();
                                let context = Context::new();
                                let eval_result =
                                    compiled_xpath_ref.evaluate(&context, document.root());

                                match eval_result {
                                    Ok(Value::String(actual_value)) => {
                                        // XPath result was explicitly a string
                                        if actual_value == expected_target {
                                            successful_urls.push(url_string.clone());
                                        } else {
                                            unsuccessful_urls.push(url_string.clone());
                                        }
                                    }
                                    Ok(Value::Nodeset(nodeset)) => {
                                        // XPath resulted in a nodeset (potentially empty)
                                        let actual_value = if nodeset.size() == 0 {
                                            // Nodeset is empty
                                            "".to_string()
                                        } else {
                                            // Get string value of the first node in document order
                                            nodeset
                                                .document_order_first()
                                                .map_or("".to_string(), |node| node.string_value())
                                        };

                                        if actual_value == expected_target {
                                            successful_urls.push(url_string.clone());
                                        } else {
                                            unsuccessful_urls.push(url_string.clone());
                                        }
                                    }
                                    Ok(_) | Err(_) => {
                                        // Handles Boolean, Number, or an evaluation Error
                                        unsuccessful_urls.push(url_string.clone());
                                    }
                                }
                            }
                            Err(_) => {
                                // Document parsing failed
                                unsuccessful_urls.push(url_string.clone());
                            }
                        }
                    } else {
                        // XPath compilation failed
                        unsuccessful_urls.push(url_string.clone());
                    }
                } else {
                    // No target specified for this heading/URL combination
                    unsuccessful_urls.push(url_string.clone());
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


    output_results
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read stdin
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    // 2. Deserialize input
    let input: InputJson = serde_json::from_str(&buffer)?;

    // --- Call the processing function ---
    let output: HashMap<String, XpathResult> = process_input(input);
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

    #[test]
    fn test_process_input_real_logic_expected_failure() {
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
        let output: HashMap<String, XpathResult> = process_input(input);

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
