use scraper::{Html, Selector};

#[derive(Debug, Clone)]
struct ATiojProblemOption {
    description: Option<String>,
    input_format: Option<String>,
    output_format: Option<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct ATiojProblem {
    description: String,
    input_format: String,
    output_format: String,
}

impl ATiojProblemOption {
    const NONE: ATiojProblemOption = ATiojProblemOption {
        description: None,
        input_format: None,
        output_format: None,
    };
}

impl From<ATiojProblemOption> for ATiojProblem {
    fn from(v: ATiojProblemOption) -> ATiojProblem {
        ATiojProblem {
            description: v.description.unwrap(),
            input_format: v.input_format.unwrap(),
            output_format: v.output_format.unwrap(),
        }
    }
}

impl ATiojProblem {
    pub fn get(target_url: &str) -> Result<Self, String> {
        println!("連線 tioj 題目中...");

        use std::process::Command;
        let Ok(output) = Command::new("curl").arg("-s").arg(target_url).output() else {
            return Err("curl 命令建立失敗".into());
        };

        if !output.status.success() {
            println!("curl 執行失敗");
            return Err("curl 執行失敗".into());
        }

        let response_text = String::from_utf8_lossy(&output.stdout);

        let document = Html::parse_document(&response_text);

        let panel_selector = Selector::parse("div.panel-default").unwrap();
        let title_selector = Selector::parse("h1.panel-title").unwrap();
        let body_selector = Selector::parse("div.panel-body").unwrap();

        let mut found_description = false;
        let mut found_input_format = false;
        let mut found_output_format = false;

        let mut problem = ATiojProblemOption::NONE;

        for panel in document.select(&panel_selector) {
            if let Some(title_element) = panel.select(&title_selector).next() {
                let title_text = title_element.text().collect::<Vec<_>>().concat();

                let title = title_text.trim();

                if title == "Description" {
                    found_description = true;
                    if let Some(body_element) = panel.select(&body_selector).next() {
                        let mut md_lines = Vec::new();
                        for child in body_element.children() {
                            if let Some(element) = scraper::ElementRef::wrap(child) {
                                let element_text: String =
                                    element.text().collect::<Vec<_>>().concat();
                                let cleaned = element_text.trim().to_string();

                                if !cleaned.is_empty() {
                                    if cleaned.starts_with("$$") && cleaned.ends_with("$$") {
                                        md_lines.push(format!("\n{}\n", cleaned));
                                    } else {
                                        md_lines.push(cleaned);
                                    }
                                }
                            }
                        }

                        problem.description = Some(md_lines.join("\n\n"));
                    }
                } else if title == "Input Format" {
                    found_input_format = true;
                    if let Some(body_element) = panel.select(&body_selector).next() {
                        let mut md_lines = Vec::new();
                        for child in body_element.children() {
                            if let Some(element) = scraper::ElementRef::wrap(child) {
                                let element_text: String =
                                    element.text().collect::<Vec<_>>().concat();
                                let cleaned = element_text.trim().to_string();

                                if !cleaned.is_empty() {
                                    if cleaned.starts_with("$$") && cleaned.ends_with("$$") {
                                        md_lines.push(format!("\n{}\n", cleaned));
                                    } else {
                                        md_lines.push(cleaned);
                                    }
                                }
                            }
                        }

                        problem.input_format = Some(md_lines.join("\n\n"));
                    }
                } else if title == "Output Format" {
                    found_output_format = true;
                    if let Some(body_element) = panel.select(&body_selector).next() {
                        let mut md_lines = Vec::new();
                        for child in body_element.children() {
                            if let Some(element) = scraper::ElementRef::wrap(child) {
                                let element_text: String =
                                    element.text().collect::<Vec<_>>().concat();
                                let cleaned = element_text.trim().to_string();

                                if !cleaned.is_empty() {
                                    if cleaned.starts_with("$$") && cleaned.ends_with("$$") {
                                        md_lines.push(format!("\n{}\n", cleaned));
                                    } else {
                                        md_lines.push(cleaned);
                                    }
                                }
                            }
                        }

                        problem.input_format = Some(md_lines.join("\n\n"));
                    }
                }
            }
        }

        if !(found_description && found_input_format && found_output_format) {
            Err("連線成功，但網頁中關於題目的資訊不全".into())
        } else {
            Ok(problem.into())
        }
    }
}
