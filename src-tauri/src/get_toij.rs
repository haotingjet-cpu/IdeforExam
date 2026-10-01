use scraper::{Html, Selector};

#[derive(Debug, Clone)]
struct ATiojProblem {
    description: Option<String>,
    input_format: Option<String>,
    output_format: Option<String>,
}
impl ATiojProblem {
    const NONE: ATiojProblem = ATiojProblem {
        description: None,
        input_format: None,
        output_format: None,
    };

    fn get() -> Result<Self, Box<dyn std::error::Error>> {
        let target_url = "https://tioj.ck.tp.edu.tw/problems/1005";

        println!("正在建立高擬真瀏覽器請求標頭...");

        use std::process::Command;
        let output = Command::new("curl").arg("-s").arg(target_url).output()?;

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

        let mut problem = Self::NONE;

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
            Err("❌ 連線成功，但該網頁中沒有找到標題為 'Description' 的區塊。".into())
        } else {
            Ok(problem)
        }
    }
}
