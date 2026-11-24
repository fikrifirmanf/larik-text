use gpui_component::highlighter::Language;
use std::path::Path;

pub fn detect(path: &Path, text: &str) -> Language {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    match name.to_ascii_lowercase().as_str() {
        "makefile" | "gnumakefile" => return Language::Make,
        "cmakelists.txt" => return Language::CMake,
        ".bashrc" | ".zshrc" | ".profile" | ".bash_profile" => return Language::Bash,
        _ => {}
    }

    let extension = path.extension().unwrap_or_default().to_string_lossy();
    let language = match extension.to_ascii_lowercase().as_str() {
        "h" => Language::C,
        "hpp" | "hxx" | "cxx" => Language::Cpp,
        "mjs" | "cjs" | "jsx" => Language::JavaScript,
        "mts" | "cts" => Language::TypeScript,
        "pyw" | "pyi" => Language::Python,
        "zsh" | "bash" => Language::Bash,
        "markdown" | "mdown" => Language::Markdown,
        extension => Language::from_str(extension),
    };
    if language != Language::Plain {
        return language;
    }

    if let Some(shebang) = text.lines().next().and_then(|line| line.strip_prefix("#!")) {
        if shebang.contains("python") {
            return Language::Python;
        }
        if shebang.contains("ruby") {
            return Language::Ruby;
        }
        if shebang
            .split(['/', ' '])
            .any(|part| matches!(part, "sh" | "bash" | "zsh"))
        {
            return Language::Bash;
        }
    }
    Language::Plain
}

pub fn label(language: Language) -> String {
    match language {
        Language::Plain => "Plain text".into(),
        Language::Cpp => "C++".into(),
        Language::CSharp => "C#".into(),
        Language::JavaScript => "JavaScript".into(),
        Language::TypeScript => "TypeScript".into(),
        Language::CMake => "CMake".into(),
        Language::GraphQL => "GraphQL".into(),
        Language::JsDoc => "JSDoc".into(),
        Language::Json
        | Language::Html
        | Language::Css
        | Language::Sql
        | Language::Yaml
        | Language::Toml
        | Language::Ejs
        | Language::Erb
        | Language::Tsx => language.name().to_ascii_uppercase(),
        language => {
            let (first, rest) = language.name().split_at(1);
            format!("{}{rest}", first.to_ascii_uppercase())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_extensions_special_names_and_shebangs() {
        assert_eq!(detect(Path::new("script.PY"), ""), Language::Python);
        assert_eq!(detect(Path::new("README.MD"), ""), Language::Markdown);
        assert_eq!(detect(Path::new("Makefile"), ""), Language::Make);
        assert_eq!(
            detect(Path::new("run"), "#!/usr/bin/env python3\n"),
            Language::Python
        );
        assert_eq!(
            detect(Path::new("example.custom"), "file content"),
            Language::Plain
        );
    }
}
