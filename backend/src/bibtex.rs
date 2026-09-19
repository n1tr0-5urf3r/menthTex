use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::Serialize;

#[derive(Clone, Debug)]
pub struct RawEntry {
    pub entry_type: String,
    pub key: String,
    pub fields: HashMap<String, String>,
    pub raw_text: String,
}

#[derive(Clone, Debug)]
pub struct AbbreviationDefinition {
    pub token: String,
    pub raw_value: String,
    pub expanded_value: String,
    pub canonical_keys: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct FormatOptions {
    pub include_string_definitions: bool,
    pub use_conferences_bib: bool,
    pub rewrite_reference_key: bool,
    pub keep_doi: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Formatted,
    Untouched,
}

#[derive(Clone, Debug, Serialize)]
pub struct FormattedEntry {
    pub entry_type: String,
    pub status: EntryStatus,
    pub key: String,
    pub original_text: String,
    pub output_text: String,
    pub warnings: Vec<String>,
    pub removed_fields: Vec<String>,
    pub normalized_fields: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct VenueSuggestion {
    pub token: String,
    pub raw_value: String,
    pub expanded_value: String,
    pub score: u16,
}

#[derive(Clone, Debug)]
pub struct ProposedDefinition {
    pub definition: AbbreviationDefinition,
    pub suggestions: Vec<VenueSuggestion>,
}

#[derive(Clone, Debug)]
pub struct FormatResult {
    pub output: String,
    pub warnings: Vec<String>,
    pub entries: Vec<FormattedEntry>,
    pub added_definitions: Vec<ProposedDefinition>,
    pub output_definitions: Vec<AbbreviationDefinition>,
    pub entry_count: usize,
    pub formatted_count: usize,
    pub untouched_count: usize,
}

#[derive(Clone, Debug)]
pub struct DefinitionCollection {
    definitions: Vec<AbbreviationDefinition>,
    lookup: HashMap<String, usize>,
    tokens: HashSet<String>,
    token_lookup: HashMap<String, usize>,
}

#[derive(Clone, Debug)]
struct SupportedFormatting {
    entry_type: String,
    key: String,
    output_text: String,
    warnings: Vec<String>,
    removed_fields: Vec<String>,
    normalized_fields: Vec<String>,
    used_definition: Option<AbbreviationDefinition>,
    added_definition: Option<AbbreviationDefinition>,
    suggestions: Vec<VenueSuggestion>,
}

#[derive(Clone, Debug)]
struct PublicationResolution {
    rendered_value: String,
    used_definition: Option<AbbreviationDefinition>,
    added_definition: Option<AbbreviationDefinition>,
    suggestions: Vec<VenueSuggestion>,
    warning: Option<String>,
}

#[derive(Clone, Debug)]
enum FormatOutcome {
    Formatted(SupportedFormatting),
    Untouched { warnings: Vec<String> },
}

impl DefinitionCollection {
    pub fn from_source(source: &str) -> Result<Self, ParseError> {
        let mut definitions = Vec::new();
        let mut lookup = HashMap::new();
        let mut tokens = HashSet::new();
        let mut token_lookup = HashMap::new();

        for (token, raw_value) in parse_string_definitions(source)? {
            let expanded_value = strip_outer_wrapping(&raw_value).trim().to_string();
            let canonical_keys = extend_with_alias_keys(
                build_publication_lookup_keys(&expanded_value),
                &token,
                &expanded_value,
            );

            let definition = AbbreviationDefinition {
                token: token.clone(),
                raw_value,
                expanded_value,
                canonical_keys: canonical_keys.clone(),
            };
            let index = definitions.len();
            for canonical in canonical_keys {
                lookup.entry(canonical).or_insert(index);
            }
            tokens.insert(token.to_ascii_uppercase());
            token_lookup.insert(token.to_ascii_uppercase(), index);
            definitions.push(definition);
        }

        Ok(Self {
            definitions,
            lookup,
            tokens,
            token_lookup,
        })
    }

    pub fn find_by_name(&self, name: &str) -> Option<AbbreviationDefinition> {
        let normalized_token = strip_outer_wrapping(name).trim().to_ascii_uppercase();
        if let Some(index) = self.token_lookup.get(&normalized_token).copied() {
            return Some(self.definitions[index].clone());
        }

        build_publication_lookup_keys(name)
            .into_iter()
            .find_map(|candidate| self.lookup.get(&candidate).copied())
            .map(|index| self.definitions[index].clone())
    }

    pub fn contains_token(&self, token: &str) -> bool {
        self.tokens.contains(&token.to_ascii_uppercase())
    }

    pub fn insert(&mut self, definition: AbbreviationDefinition) {
        let index = self.definitions.len();
        for canonical in &definition.canonical_keys {
            self.lookup.entry(canonical.clone()).or_insert(index);
        }
        let normalized_token = definition.token.to_ascii_uppercase();
        self.tokens.insert(normalized_token.clone());
        self.token_lookup.insert(normalized_token, index);
        self.definitions.push(definition);
    }

    pub fn definitions(&self) -> &[AbbreviationDefinition] {
        &self.definitions
    }

    pub fn similar_by_name(&self, name: &str, limit: usize) -> Vec<VenueSuggestion> {
        let cleaned_target = clean_publication_name(name);
        let target_canonical = canonicalize_publication_name(&cleaned_target);
        let target_tokens = tokenize_publication_similarity_tokens(&cleaned_target);
        if target_canonical.is_empty() || target_tokens.is_empty() {
            return Vec::new();
        }

        let mut suggestions = self
            .definitions
            .iter()
            .filter_map(|definition| {
                let cleaned_candidate = clean_publication_name(&definition.expanded_value);
                let candidate_canonical = canonicalize_publication_name(&cleaned_candidate);
                let candidate_tokens = tokenize_publication_similarity_tokens(&cleaned_candidate);
                let score = similarity_score(
                    &target_canonical,
                    &target_tokens,
                    &candidate_canonical,
                    &candidate_tokens,
                );
                if score < 45 {
                    return None;
                }

                Some(VenueSuggestion {
                    token: definition.token.clone(),
                    raw_value: definition.raw_value.clone(),
                    expanded_value: definition.expanded_value.clone(),
                    score,
                })
            })
            .collect::<Vec<_>>();

        suggestions.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.token.cmp(&right.token))
        });
        suggestions.truncate(limit);
        suggestions
    }
}

fn parse_string_definitions(source: &str) -> Result<Vec<(String, String)>, ParseError> {
    let mut definitions = Vec::new();
    let mut current = String::new();
    let mut collecting = false;
    let mut balance = 0isize;

    for line in source.lines() {
        let trimmed = line.trim_start();
        if !collecting {
            if !trimmed.to_ascii_lowercase().starts_with("@string") {
                continue;
            }
            collecting = true;
        }

        current.push_str(line);
        current.push('\n');
        balance += brace_delta(line);

        if collecting && balance <= 0 {
            let (token, raw_value) = parse_string_definition_entry(current.trim())?;
            definitions.push((token, raw_value));
            current.clear();
            collecting = false;
            balance = 0;
        }
    }

    if collecting {
        return Err(ParseError::new(
            "Unterminated @STRING definition in conferences.bib.",
        ));
    }

    Ok(definitions)
}

fn parse_string_definition_entry(input: &str) -> Result<(String, String), ParseError> {
    let start = input
        .find('{')
        .or_else(|| input.find('('))
        .ok_or_else(|| ParseError::new("Invalid @STRING definition."))?;
    let end = input
        .rfind('}')
        .or_else(|| input.rfind(')'))
        .ok_or_else(|| ParseError::new("Invalid @STRING definition."))?;

    if start + 1 >= end {
        return Err(ParseError::new("Empty @STRING definition."));
    }

    let inner = input[start + 1..end].trim();
    let (token, value) = split_assignment_without_quotes(inner)
        .ok_or_else(|| ParseError::new("Invalid @STRING definition assignment."))?;
    Ok((
        token.trim().to_string(),
        value.trim().trim_end_matches(',').trim().to_string(),
    ))
}

fn split_assignment_without_quotes(segment: &str) -> Option<(&str, &str)> {
    let bytes = segment.as_bytes();
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;

    for (index, byte) in bytes.iter().copied().enumerate() {
        match byte {
            b'{' => brace_depth += 1,
            b'}' if brace_depth > 0 => brace_depth -= 1,
            b'(' => paren_depth += 1,
            b')' if paren_depth > 0 => paren_depth -= 1,
            b'=' if brace_depth == 0 && paren_depth == 0 => {
                return Some((&segment[..index], &segment[index + 1..]));
            }
            _ => {}
        }
    }

    None
}

fn brace_delta(line: &str) -> isize {
    let mut delta = 0isize;
    for byte in line.as_bytes() {
        match *byte {
            b'{' => delta += 1,
            b'}' => delta -= 1,
            _ => {}
        }
    }

    delta
}

#[derive(Debug, Clone)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

pub fn parse_bibtex_document(input: &str) -> Result<Vec<RawEntry>, ParseError> {
    let bytes = input.as_bytes();
    let mut index = 0usize;
    let mut entries = Vec::new();

    while let Some(relative) = input[index..].find('@') {
        index += relative;
        let (entry, next_index) = parse_entry_at(input, bytes, index)?;
        entries.push(entry);
        index = next_index;
    }

    Ok(entries)
}

fn parse_entry_at(
    input: &str,
    bytes: &[u8],
    start: usize,
) -> Result<(RawEntry, usize), ParseError> {
    let mut index = start + 1;
    index = skip_ascii_whitespace(bytes, index);
    let type_start = index;

    while index < bytes.len() && is_ident_byte(bytes[index]) {
        index += 1;
    }

    if type_start == index {
        return Err(ParseError::new("Expected BibTeX entry type after '@'."));
    }

    let entry_type = input[type_start..index].trim().to_string();
    index = skip_ascii_whitespace(bytes, index);

    if index >= bytes.len() || (bytes[index] != b'{' && bytes[index] != b'(') {
        return Err(ParseError::new(format!(
            "Expected '{{' or '(' after entry type '{entry_type}'."
        )));
    }

    let open = bytes[index];
    let close = if open == b'{' { b'}' } else { b')' };
    let content_start = index + 1;
    index += 1;

    let mut depth = 1usize;
    let mut brace_depth = 0usize;
    let mut in_quotes = false;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if in_quotes {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_quotes = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' if !is_escaped(bytes, index) => in_quotes = true,
            b'{' if open == b'(' => brace_depth += 1,
            b'}' if open == b'(' && brace_depth > 0 => brace_depth -= 1,
            b if b == open && brace_depth == 0 => depth += 1,
            b if b == close && brace_depth == 0 => {
                depth -= 1;
                if depth == 0 {
                    let content = input[content_start..index].trim();
                    let mut entry = parse_entry_content(&entry_type, content)?;
                    entry.raw_text = input[start..index + 1].to_string();
                    return Ok((entry, index + 1));
                }
            }
            _ => {}
        }
        index += 1;
    }

    Err(ParseError::new(format!(
        "Unterminated BibTeX entry starting with '@{entry_type}'."
    )))
}

fn parse_entry_content(entry_type: &str, content: &str) -> Result<RawEntry, ParseError> {
    if entry_type.eq_ignore_ascii_case("comment") || entry_type.eq_ignore_ascii_case("preamble") {
        return Ok(RawEntry {
            entry_type: entry_type.to_string(),
            key: content.trim().to_string(),
            fields: HashMap::new(),
            raw_text: String::new(),
        });
    }

    if entry_type.eq_ignore_ascii_case("string") {
        let (token, raw_value) = split_assignment(content)
            .ok_or_else(|| ParseError::new("Invalid @STRING definition."))?;
        let token = token.trim().to_string();
        let raw_value = raw_value.trim().trim_end_matches(',').trim().to_string();
        let mut fields = HashMap::new();
        fields.insert("__token__".to_string(), token.clone());
        fields.insert("__value__".to_string(), raw_value);
        return Ok(RawEntry {
            entry_type: entry_type.to_string(),
            key: token,
            fields,
            raw_text: String::new(),
        });
    }

    let parts = split_top_level(content, b',');
    if parts.is_empty() {
        return Err(ParseError::new("Empty BibTeX entry body."));
    }

    let key = parts[0].trim().to_string();
    let mut fields = HashMap::new();

    for segment in parts.into_iter().skip(1) {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (field_name, raw_value) = split_assignment(trimmed)
            .ok_or_else(|| ParseError::new(format!("Invalid field assignment: '{trimmed}'")))?;
        fields.insert(
            field_name.trim().to_ascii_lowercase(),
            raw_value.trim().trim_end_matches(',').trim().to_string(),
        );
    }

    Ok(RawEntry {
        entry_type: entry_type.to_string(),
        key,
        fields,
        raw_text: String::new(),
    })
}

fn split_assignment(segment: &str) -> Option<(&str, &str)> {
    let bytes = segment.as_bytes();
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut in_quotes = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().copied().enumerate() {
        if in_quotes {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_quotes = false;
            }
            continue;
        }

        match byte {
            b'"' if !is_escaped(bytes, index) => in_quotes = true,
            b'{' => brace_depth += 1,
            b'}' if brace_depth > 0 => brace_depth -= 1,
            b'(' => paren_depth += 1,
            b')' if paren_depth > 0 => paren_depth -= 1,
            b'=' if brace_depth == 0 && paren_depth == 0 => {
                return Some((&segment[..index], &segment[index + 1..]));
            }
            _ => {}
        }
    }

    None
}

fn split_top_level(input: &str, separator: u8) -> Vec<&str> {
    let bytes = input.as_bytes();
    let mut parts = Vec::new();
    let mut last = 0usize;
    let mut brace_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut in_quotes = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().copied().enumerate() {
        if in_quotes {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_quotes = false;
            }
            continue;
        }

        match byte {
            b'"' if !is_escaped(bytes, index) => in_quotes = true,
            b'{' => brace_depth += 1,
            b'}' if brace_depth > 0 => brace_depth -= 1,
            b'(' => paren_depth += 1,
            b')' if paren_depth > 0 => paren_depth -= 1,
            b if b == separator && brace_depth == 0 && paren_depth == 0 => {
                parts.push(&input[last..index]);
                last = index + 1;
            }
            _ => {}
        }
    }

    parts.push(&input[last..]);
    parts
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    index > 0 && bytes[index - 1] == b'\\'
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

pub fn format_entries(
    entries: &[RawEntry],
    definitions: &mut DefinitionCollection,
    options: &FormatOptions,
) -> Result<FormatResult, ParseError> {
    if entries.is_empty() {
        return Err(ParseError::new(
            "No BibTeX entries were found in the input.",
        ));
    }

    let mut generated_keys: HashMap<String, usize> = HashMap::new();
    let mut formatted_entries = Vec::new();
    let mut added_definitions = Vec::new();
    let mut output_definitions = Vec::new();
    let mut warnings = Vec::new();
    let mut string_definitions_for_output = Vec::new();
    let mut seen_definition_tokens = HashSet::new();
    let input_string_tokens = entries
        .iter()
        .filter(|entry| entry.entry_type.eq_ignore_ascii_case("string"))
        .map(|entry| entry.key.to_ascii_uppercase())
        .collect::<HashSet<_>>();
    let mut formatted_count = 0usize;
    let mut untouched_count = 0usize;

    for entry in entries {
        if let Some(supported_type) = supported_entry_type(&entry.entry_type) {
            let outcome = format_supported_entry(
                entry,
                supported_type,
                definitions,
                &mut generated_keys,
                options,
            );

            match outcome? {
                FormatOutcome::Formatted(formatted) => {
                    warnings.extend(formatted.warnings.clone());

                    if let Some(definition) = &formatted.added_definition {
                        definitions.insert(definition.clone());
                        added_definitions.push(ProposedDefinition {
                            definition: definition.clone(),
                            suggestions: formatted.suggestions.clone(),
                        });
                    }

                    let maybe_definition = formatted
                        .added_definition
                        .clone()
                        .or_else(|| formatted.used_definition.clone());

                    if let Some(definition) = maybe_definition {
                        let token = definition.token.to_ascii_uppercase();
                        if !input_string_tokens.contains(&token)
                            && seen_definition_tokens.insert(token.clone())
                        {
                            output_definitions.push(definition.clone());
                            if options.include_string_definitions {
                                string_definitions_for_output
                                    .push(format_string_definition(&definition));
                            }
                        }
                    }

                    formatted_entries.push(FormattedEntry {
                        entry_type: formatted.entry_type,
                        status: EntryStatus::Formatted,
                        key: formatted.key,
                        original_text: entry.raw_text.clone(),
                        output_text: formatted.output_text,
                        warnings: formatted.warnings,
                        removed_fields: formatted.removed_fields,
                        normalized_fields: formatted.normalized_fields,
                    });
                    formatted_count += 1;
                }
                FormatOutcome::Untouched {
                    warnings: entry_warnings,
                } => {
                    warnings.extend(entry_warnings.clone());
                    formatted_entries.push(FormattedEntry {
                        entry_type: entry.entry_type.to_ascii_lowercase(),
                        status: EntryStatus::Untouched,
                        key: entry.key.clone(),
                        original_text: entry.raw_text.clone(),
                        output_text: entry.raw_text.clone(),
                        warnings: entry_warnings,
                        removed_fields: Vec::new(),
                        normalized_fields: Vec::new(),
                    });
                    untouched_count += 1;
                }
            }
        } else {
            formatted_entries.push(FormattedEntry {
                entry_type: entry.entry_type.to_ascii_lowercase(),
                status: EntryStatus::Untouched,
                key: entry.key.clone(),
                original_text: entry.raw_text.clone(),
                output_text: entry.raw_text.clone(),
                warnings: Vec::new(),
                removed_fields: Vec::new(),
                normalized_fields: Vec::new(),
            });
            untouched_count += 1;
        }
    }

    let mut sections = Vec::new();
    if !string_definitions_for_output.is_empty() {
        sections.push(string_definitions_for_output.join("\n"));
    }
    sections.push(
        formatted_entries
            .iter()
            .map(|entry| entry.output_text.clone())
            .collect::<Vec<_>>()
            .join("\n\n"),
    );

    Ok(FormatResult {
        output: sections.join("\n\n"),
        warnings,
        entry_count: formatted_entries.len(),
        formatted_count,
        untouched_count,
        entries: formatted_entries,
        added_definitions,
        output_definitions,
    })
}

fn supported_entry_type(entry_type: &str) -> Option<String> {
    let normalized = entry_type.to_ascii_lowercase();
    match normalized.as_str() {
        "inproceedings" | "article" | "misc" | "techreport" => Some(normalized),
        _ => None,
    }
}

fn format_supported_entry(
    entry: &RawEntry,
    supported_type: String,
    definitions: &DefinitionCollection,
    generated_keys: &mut HashMap<String, usize>,
    options: &FormatOptions,
) -> Result<FormatOutcome, ParseError> {
    if supported_type == "misc" || supported_type == "techreport" {
        return format_rfc_entry(entry, options);
    }

    if supported_type == "article" && is_standard_like_entry(entry) {
        return format_standard_entry(entry, definitions, options);
    }

    let author_raw = match entry.fields.get("author") {
        Some(author) if !strip_outer_wrapping(author).trim().is_empty() => author.as_str(),
        _ => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing a usable author field.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };
    let authors = split_authors(author_raw);
    if authors.is_empty() {
        return Ok(FormatOutcome::Untouched {
            warnings: vec![format!(
                "Left entry '{}' untouched because '@{}' does not contain a usable author list.",
                entry.key, entry.entry_type
            )],
        });
    }

    let year_raw = match entry.fields.get("year") {
        Some(year) => year.as_str(),
        None => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing required field 'year'.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };
    let year = normalize_year(year_raw);
    let title_raw = match entry.fields.get("title") {
        Some(title) => title.as_str(),
        None => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing required field 'title'.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };
    let title = normalize_title(title_raw);
    let key = if options.rewrite_reference_key {
        generate_unique_key(&authors, &year, generated_keys)
    } else {
        entry.key.clone()
    };
    let author_value = authors.join(" and ");
    let month = entry
        .fields
        .get("month")
        .map(|value| normalize_month(value))
        .filter(|value| !value.is_empty());
    let numpages = entry
        .fields
        .get("numpages")
        .or_else(|| entry.fields.get("num_pages"))
        .map(|value| strip_outer_wrapping(value).trim().to_string())
        .filter(|value| !value.is_empty());

    let mut warnings = Vec::new();
    let mut used_definition = None;
    let mut added_definition = None;
    let mut suggestions = Vec::new();
    let mut normalized_fields = Vec::new();
    let mut kept_fields = HashSet::new();
    let mut rendered_fields = vec![
        format!("    author = {{{author_value}}}"),
        format!("    title = {{{{{title}}}}}"),
        format!("    year = {year}"),
    ];

    kept_fields.insert("author");
    kept_fields.insert("title");
    kept_fields.insert("year");

    let normalized_author = strip_outer_wrapping(author_raw)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized_author != author_value {
        push_unique(&mut normalized_fields, "author");
    }

    if strip_outer_wrapping(title_raw) != title {
        push_unique(&mut normalized_fields, "title");
    }

    if strip_outer_wrapping(year_raw) != year {
        push_unique(&mut normalized_fields, "year");
    }

    if entry.key != key {
        push_unique(&mut normalized_fields, "key");
    }

    if supported_type == "inproceedings" {
        let raw_booktitle = match entry.fields.get("booktitle") {
            Some(booktitle) => booktitle.as_str(),
            None => {
                return Ok(FormatOutcome::Untouched {
                    warnings: vec![format!(
                        "Left entry '{}' untouched because '@{}' is missing required field 'booktitle'.",
                        entry.key, entry.entry_type
                    )],
                });
            }
        };
        let publication_name = clean_publication_name(raw_booktitle);
        let publication = resolve_publication_reference(
            &publication_name,
            definitions,
            "conference",
            options.use_conferences_bib,
        );
        if let Some(warning) = &publication.warning {
            warnings.push(warning.clone());
        }
        used_definition = publication.used_definition.clone();
        added_definition = publication.added_definition.clone();
        suggestions = publication.suggestions.clone();
        rendered_fields.push(format!("    booktitle = {}", publication.rendered_value));
        kept_fields.insert("booktitle");

        if strip_outer_wrapping(raw_booktitle).trim()
            != strip_outer_wrapping(&publication.rendered_value).trim()
        {
            push_unique(&mut normalized_fields, "booktitle");
        }

        if let Some(pages_raw) = entry.fields.get("pages") {
            if let Some(pages) = normalize_pages(pages_raw, numpages.as_deref(), true) {
                rendered_fields.push(format!("    pages = {{{pages}}}"));
                kept_fields.insert("pages");
                let original_pages = normalize_unicode_dashes(&strip_outer_wrapping(pages_raw));
                if original_pages.replace('-', "--") != pages {
                    push_unique(&mut normalized_fields, "pages");
                }
            }
        }

        if let Some(month) = month {
            rendered_fields.push(format!("    month = {month}"));
            kept_fields.insert("month");
            if let Some(month_raw) = entry.fields.get("month") {
                if normalize_month(month_raw)
                    != strip_outer_wrapping(month_raw).to_ascii_lowercase()
                {
                    push_unique(&mut normalized_fields, "month");
                }
            }
        }
    } else {
        let mut output_entry_type = "article".to_string();
        if let Some(volume) = entry
            .fields
            .get("volume")
            .map(|value| strip_outer_wrapping(value).trim().to_string())
            .filter(|value| !value.is_empty())
        {
            rendered_fields.push(format!("    volume = {volume}"));
            kept_fields.insert("volume");
        }

        if let Some(number) = entry
            .fields
            .get("number")
            .map(|value| strip_outer_wrapping(value).trim().to_string())
            .filter(|value| !value.is_empty())
        {
            rendered_fields.push(format!("    number = {number}"));
            kept_fields.insert("number");
        }

        if let Some(raw_journal) = entry.fields.get("journal").map(String::as_str) {
            let publication_name = clean_publication_name(raw_journal);
            let publication = resolve_publication_reference(
                &publication_name,
                definitions,
                "journal",
                options.use_conferences_bib,
            );
            if let Some(warning) = &publication.warning {
                warnings.push(warning.clone());
            }
            used_definition = publication.used_definition.clone();
            added_definition = publication.added_definition.clone();
            suggestions = publication.suggestions.clone();
            rendered_fields.push(format!("    journal = {}", publication.rendered_value));
            kept_fields.insert("journal");

            if strip_outer_wrapping(raw_journal).trim()
                != strip_outer_wrapping(&publication.rendered_value).trim()
            {
                push_unique(&mut normalized_fields, "journal");
            }

            if let Some(pages_raw) = entry.fields.get("pages") {
                if let Some(pages) = normalize_pages(pages_raw, numpages.as_deref(), false) {
                    rendered_fields.push(format!("    pages = {{{pages}}}"));
                    kept_fields.insert("pages");
                    let original_pages = normalize_unicode_dashes(&strip_outer_wrapping(pages_raw));
                    if original_pages.replace('-', "--") != pages {
                        push_unique(&mut normalized_fields, "pages");
                    }
                }
            }
        } else {
            output_entry_type = "misc".to_string();
            warnings.push(format!(
                "Formatted entry '{}' as '@misc' because '@{}' is missing required field 'journal'.",
                entry.key, entry.entry_type
            ));
            rendered_fields.retain(|field| {
                !(field.starts_with("    volume =") || field.starts_with("    number ="))
            });
        }

        if let Some(month) = month {
            rendered_fields.push(format!("    month = {month}"));
            kept_fields.insert("month");
            if let Some(month_raw) = entry.fields.get("month") {
                if normalize_month(month_raw)
                    != strip_outer_wrapping(month_raw).to_ascii_lowercase()
                {
                    push_unique(&mut normalized_fields, "month");
                }
            }
        }

        if output_entry_type == "article" {
            if let Some(numpages) = numpages {
                rendered_fields.push(format!("    numpages = {numpages}"));
                kept_fields.insert("numpages");
                if entry.fields.contains_key("num_pages") {
                    push_unique(&mut normalized_fields, "numpages");
                }
            }
        }

        append_doi_field(entry, options, &mut rendered_fields, &mut kept_fields);

        let mut removed_fields = entry
            .fields
            .keys()
            .filter_map(|field| match field.as_str() {
                "__token__" | "__value__" => None,
                "num_pages" if kept_fields.contains("numpages") => None,
                name if kept_fields.contains(name) => None,
                name => Some(name.to_string()),
            })
            .collect::<Vec<_>>();
        removed_fields.sort();

        let output_text = format!(
            "@{}{{{},\n{}\n}}",
            output_entry_type,
            key,
            rendered_fields.join(",\n")
        );

        return Ok(FormatOutcome::Formatted(SupportedFormatting {
            entry_type: output_entry_type,
            key,
            output_text,
            warnings,
            removed_fields,
            normalized_fields,
            used_definition,
            added_definition,
            suggestions,
        }));
    }

    append_doi_field(entry, options, &mut rendered_fields, &mut kept_fields);

    let mut removed_fields = entry
        .fields
        .keys()
        .filter_map(|field| match field.as_str() {
            "__token__" | "__value__" => None,
            "num_pages" if kept_fields.contains("numpages") => None,
            name if kept_fields.contains(name) => None,
            name => Some(name.to_string()),
        })
        .collect::<Vec<_>>();
    removed_fields.sort();

    let output_text = format!(
        "@{}{{{},\n{}\n}}",
        supported_type,
        key,
        rendered_fields.join(",\n")
    );

    Ok(FormatOutcome::Formatted(SupportedFormatting {
        entry_type: supported_type,
        key,
        output_text,
        warnings,
        removed_fields,
        normalized_fields,
        used_definition,
        added_definition,
        suggestions,
    }))
}

fn format_standard_entry(
    entry: &RawEntry,
    _definitions: &DefinitionCollection,
    options: &FormatOptions,
) -> Result<FormatOutcome, ParseError> {
    let year_raw = match entry.fields.get("year") {
        Some(year) => year.as_str(),
        None => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing required field 'year'.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };
    let title_raw = match entry.fields.get("title") {
        Some(title) => title.as_str(),
        None => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing required field 'title'.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };
    let raw_journal = match entry.fields.get("journal") {
        Some(journal) => journal.as_str(),
        None => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing required field 'journal'.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };

    let year = normalize_year(year_raw);
    let title = normalize_title(title_raw);
    let month = entry
        .fields
        .get("month")
        .map(|value| normalize_month(value))
        .filter(|value| !value.is_empty());

    let warnings = vec![format!(
        "Formatted entry '{}' as a standard-like '@article' without generating a new key because no usable author list was provided.",
        entry.key
    )];

    let mut normalized_fields = Vec::new();
    let mut kept_fields = HashSet::new();
    let journal_value = normalize_unicode_dashes(strip_outer_wrapping(raw_journal).trim());
    let mut rendered_fields = vec![
        format!("    title = {{{{{title}}}}}"),
        format!("    year = {year}"),
        format!("    journal = {{{journal_value}}}"),
    ];

    kept_fields.insert("title");
    kept_fields.insert("year");
    kept_fields.insert("journal");

    if strip_outer_wrapping(title_raw) != title {
        push_unique(&mut normalized_fields, "title");
    }
    if strip_outer_wrapping(year_raw) != year {
        push_unique(&mut normalized_fields, "year");
    }
    if let Some(month) = month {
        rendered_fields.push(format!("    month = {month}"));
        kept_fields.insert("month");
        if let Some(month_raw) = entry.fields.get("month") {
            if normalize_month(month_raw) != strip_outer_wrapping(month_raw).to_ascii_lowercase() {
                push_unique(&mut normalized_fields, "month");
            }
        }
    }

    append_doi_field(entry, options, &mut rendered_fields, &mut kept_fields);

    let mut removed_fields = entry
        .fields
        .keys()
        .filter_map(|field| match field.as_str() {
            "__token__" | "__value__" => None,
            name if kept_fields.contains(name) => None,
            name => Some(name.to_string()),
        })
        .collect::<Vec<_>>();
    removed_fields.sort();

    let output_text = format!(
        "@article{{{},\n{}\n}}",
        entry.key,
        rendered_fields.join(",\n")
    );

    Ok(FormatOutcome::Formatted(SupportedFormatting {
        entry_type: "article".to_string(),
        key: entry.key.clone(),
        output_text,
        warnings,
        removed_fields,
        normalized_fields,
        used_definition: None,
        added_definition: None,
        suggestions: Vec::new(),
    }))
}

fn format_rfc_entry(
    entry: &RawEntry,
    options: &FormatOptions,
) -> Result<FormatOutcome, ParseError> {
    if !is_rfc_entry(entry) {
        return Ok(FormatOutcome::Untouched {
            warnings: Vec::new(),
        });
    }

    let author_raw = match entry.fields.get("author") {
        Some(author) if !strip_outer_wrapping(author).trim().is_empty() => author.as_str(),
        _ => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing a usable author field.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };
    let authors = split_authors(author_raw);
    if authors.is_empty() {
        return Ok(FormatOutcome::Untouched {
            warnings: vec![format!(
                "Left entry '{}' untouched because '@{}' does not contain a usable author list.",
                entry.key, entry.entry_type
            )],
        });
    }

    let number = match entry
        .fields
        .get("number")
        .map(|value| strip_outer_wrapping(value).trim().to_string())
        .filter(|value| !value.is_empty())
    {
        Some(number) => number,
        None => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing required field 'number'.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };

    let pagetotal = match entry
        .fields
        .get("pagetotal")
        .map(|value| strip_outer_wrapping(value).trim().to_string())
        .filter(|value| !value.is_empty())
    {
        Some(pagetotal) => pagetotal,
        None => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing required field 'pagetotal'.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };

    let year_raw = match entry.fields.get("year") {
        Some(year) => year.as_str(),
        None => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing required field 'year'.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };
    let title_raw = match entry.fields.get("title") {
        Some(title) => title.as_str(),
        None => {
            return Ok(FormatOutcome::Untouched {
                warnings: vec![format!(
                    "Left entry '{}' untouched because '@{}' is missing required field 'title'.",
                    entry.key, entry.entry_type
                )],
            });
        }
    };
    let title = normalize_rfc_title(&number, title_raw);
    let year = normalize_year(year_raw);
    let month = entry
        .fields
        .get("month")
        .map(|value| normalize_month(value))
        .filter(|value| !value.is_empty());

    let author_value = authors.join(" and ");
    let key = entry.key.clone();
    let mut normalized_fields = Vec::new();
    let mut kept_fields = HashSet::new();
    let mut rendered_fields = vec![
        "  series      = {Request for Comments}".to_string(),
        format!("  number      = {number}"),
        "  institution = {IETF}".to_string(),
        "  type        = {RFC}".to_string(),
        format!("  author      = {{{author_value}}}"),
        format!("  title       = {{{{{title}}}}}"),
        format!("  pagetotal   = {pagetotal}"),
        format!("  year        = {year}"),
    ];

    kept_fields.insert("series");
    kept_fields.insert("number");
    kept_fields.insert("institution");
    kept_fields.insert("type");
    kept_fields.insert("author");
    kept_fields.insert("title");
    kept_fields.insert("pagetotal");
    kept_fields.insert("year");

    let normalized_author = strip_outer_wrapping(author_raw)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized_author != author_value {
        push_unique(&mut normalized_fields, "author");
    }

    if strip_outer_wrapping(title_raw) != title {
        push_unique(&mut normalized_fields, "title");
    }

    if strip_outer_wrapping(year_raw) != year {
        push_unique(&mut normalized_fields, "year");
    }

    if entry
        .fields
        .get("series")
        .map(|value| canonicalize_publication_name(&strip_outer_wrapping(value)))
        .unwrap_or_default()
        != canonicalize_publication_name("Request for Comments")
    {
        push_unique(&mut normalized_fields, "series");
    }

    if entry
        .fields
        .get("institution")
        .map(|value| strip_outer_wrapping(value).trim().to_ascii_uppercase())
        .as_deref()
        != Some("IETF")
    {
        push_unique(&mut normalized_fields, "institution");
    }

    if entry
        .fields
        .get("type")
        .map(|value| strip_outer_wrapping(value).trim().to_ascii_uppercase())
        .as_deref()
        != Some("RFC")
    {
        push_unique(&mut normalized_fields, "type");
    }

    if let Some(month) = month {
        rendered_fields.push(format!("  month       = {month}"));
        kept_fields.insert("month");
        if let Some(month_raw) = entry.fields.get("month") {
            if normalize_month(month_raw) != strip_outer_wrapping(month_raw).to_ascii_lowercase() {
                push_unique(&mut normalized_fields, "month");
            }
        }
    }

    if options.keep_doi {
        if let Some(doi) = doi_field_value(entry) {
            rendered_fields.push(format!("  doi         = {{{doi}}}"));
            kept_fields.insert("doi");
        }
    }

    let mut removed_fields = entry
        .fields
        .keys()
        .filter_map(|field| match field.as_str() {
            "__token__" | "__value__" => None,
            name if kept_fields.contains(name) => None,
            name => Some(name.to_string()),
        })
        .collect::<Vec<_>>();
    removed_fields.sort();

    let output_text = format!("@techreport{{{},\n{}\n}}", key, rendered_fields.join(",\n"));

    Ok(FormatOutcome::Formatted(SupportedFormatting {
        entry_type: "techreport".to_string(),
        key,
        output_text,
        warnings: Vec::new(),
        removed_fields,
        normalized_fields,
        used_definition: None,
        added_definition: None,
        suggestions: Vec::new(),
    }))
}

fn doi_field_value(entry: &RawEntry) -> Option<String> {
    entry
        .fields
        .get("doi")
        .map(|value| strip_outer_wrapping(value).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn append_doi_field(
    entry: &RawEntry,
    options: &FormatOptions,
    rendered_fields: &mut Vec<String>,
    kept_fields: &mut HashSet<&str>,
) {
    if !options.keep_doi {
        return;
    }

    if let Some(doi) = doi_field_value(entry) {
        rendered_fields.push(format!("    doi = {{{doi}}}"));
        kept_fields.insert("doi");
    }
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn is_rfc_entry(entry: &RawEntry) -> bool {
    if !entry.entry_type.eq_ignore_ascii_case("misc")
        && !entry.entry_type.eq_ignore_ascii_case("techreport")
    {
        return false;
    }

    let series_is_rfc = entry
        .fields
        .get("series")
        .map(|value| canonicalize_publication_name(&strip_outer_wrapping(value)))
        .map(|value| value == "request for comments")
        .unwrap_or(false);
    let howpublished_is_rfc = entry
        .fields
        .get("howpublished")
        .map(|value| strip_outer_wrapping(value).to_ascii_lowercase())
        .map(|value| value.starts_with("rfc "))
        .unwrap_or(false);
    let key_is_rfc = entry.key.to_ascii_lowercase().starts_with("rfc");

    series_is_rfc || howpublished_is_rfc || key_is_rfc
}

fn is_standard_like_entry(entry: &RawEntry) -> bool {
    if !entry.entry_type.eq_ignore_ascii_case("article") {
        return false;
    }

    let title_is_standard = entry
        .fields
        .get("title")
        .map(|value| strip_outer_wrapping(value).to_ascii_lowercase())
        .map(|value| value.contains("ieee standard"))
        .unwrap_or(false);
    let journal_is_standard = entry
        .fields
        .get("journal")
        .map(|value| strip_outer_wrapping(value).to_ascii_lowercase())
        .map(|value| value.contains("ieee std") || value.contains("ansi/ieee std"))
        .unwrap_or(false);

    title_is_standard || journal_is_standard
}

pub fn format_string_definition(definition: &AbbreviationDefinition) -> String {
    format!("@STRING{{{} = {}}}", definition.token, definition.raw_value)
}

pub fn format_definition_source(definitions: &[AbbreviationDefinition]) -> String {
    if definitions.is_empty() {
        String::new()
    } else {
        format!(
            "{}\n",
            definitions
                .iter()
                .map(format_string_definition)
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

pub fn definition_from_parts(token: &str, raw_value: &str) -> AbbreviationDefinition {
    let normalized_token = token.trim().to_string();
    let normalized_raw_value = raw_value.trim().to_string();
    let expanded_value = strip_outer_wrapping(&normalized_raw_value)
        .trim()
        .to_string();
    let canonical_keys = extend_with_alias_keys(
        build_publication_lookup_keys(&expanded_value),
        &normalized_token,
        &expanded_value,
    );

    AbbreviationDefinition {
        token: normalized_token,
        raw_value: normalized_raw_value,
        expanded_value,
        canonical_keys,
    }
}

pub fn definition_from_expanded_value(token: &str, expanded_value: &str) -> AbbreviationDefinition {
    let normalized_token = token.trim().to_string();
    let normalized_expanded_value =
        ensure_publication_name_includes_token(expanded_value.trim(), &normalized_token);
    let raw_value = format!("{{{{{normalized_expanded_value}}}}}");
    definition_from_parts(&normalized_token, &raw_value)
}

pub fn create_definition(
    publication_name: &str,
    definitions: &DefinitionCollection,
) -> AbbreviationDefinition {
    let base_publication_name = clean_publication_name(publication_name);
    let token = generate_unique_abbreviation_token(&base_publication_name, definitions);
    let expanded_value = ensure_publication_name_includes_token(&base_publication_name, &token);
    let raw_value = format!("{{{{{expanded_value}}}}}");
    let canonical_keys = extend_with_alias_keys(
        build_publication_lookup_keys(&expanded_value),
        &token,
        &expanded_value,
    );

    AbbreviationDefinition {
        token,
        raw_value,
        expanded_value,
        canonical_keys,
    }
}

fn ensure_publication_name_includes_token(publication_name: &str, token: &str) -> String {
    let trimmed_name = publication_name.trim();
    let suffix = format!("({token})");
    if trimmed_name.ends_with(&suffix) {
        trimmed_name.to_string()
    } else {
        format!("{trimmed_name} {suffix}")
    }
}

fn resolve_publication_reference(
    publication_name: &str,
    definitions: &DefinitionCollection,
    publication_kind: &str,
    use_conferences_bib: bool,
) -> PublicationResolution {
    if !use_conferences_bib {
        return PublicationResolution {
            rendered_value: format!("{{{publication_name}}}"),
            used_definition: None,
            added_definition: None,
            suggestions: Vec::new(),
            warning: None,
        };
    }

    if let Some(definition) = definitions.find_by_name(publication_name) {
        return PublicationResolution {
            rendered_value: definition.token.clone(),
            used_definition: Some(definition),
            added_definition: None,
            suggestions: Vec::new(),
            warning: None,
        };
    }

    if looks_like_abbreviated_publication_name(publication_name) {
        return PublicationResolution {
            rendered_value: format!("{{{publication_name}}}"),
            used_definition: None,
            added_definition: None,
            suggestions: definitions.similar_by_name(publication_name, 3),
            warning: Some(format!(
                "Kept {} '{}' as a literal value because it looks like an abbreviation and no existing @STRING definition matched it.",
                publication_kind, publication_name
            )),
        };
    }

    let created = create_definition(publication_name, definitions);
    PublicationResolution {
        rendered_value: created.token.clone(),
        used_definition: None,
        added_definition: Some(created.clone()),
        suggestions: definitions.similar_by_name(publication_name, 3),
        warning: None,
    }
}

fn generate_unique_abbreviation_token(
    publication_name: &str,
    definitions: &DefinitionCollection,
) -> String {
    let candidate = extract_parenthetical_abbreviation(publication_name)
        .or_else(|| Some(generate_acronym(publication_name)))
        .filter(|token| !token.is_empty())
        .unwrap_or_else(|| "PUB".to_string());

    if !definitions.contains_token(&candidate) {
        return candidate;
    }

    let mut suffix = 2usize;
    loop {
        let token = format!("{candidate}{suffix}");
        if !definitions.contains_token(&token) {
            return token;
        }
        suffix += 1;
    }
}

fn extract_parenthetical_abbreviation(name: &str) -> Option<String> {
    let trimmed = strip_outer_wrapping(name);
    let end = trimmed.rfind(')')?;
    let start = trimmed[..end].rfind('(')?;
    let candidate = trimmed[start + 1..end]
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect::<String>()
        .to_ascii_uppercase();
    if candidate.len() >= 2 {
        Some(candidate)
    } else {
        None
    }
}

fn generate_acronym(name: &str) -> String {
    let stopwords = [
        "a",
        "an",
        "and",
        "at",
        "conference",
        "communications",
        "for",
        "in",
        "international",
        "journal",
        "of",
        "on",
        "review",
        "society",
        "symposium",
        "the",
        "transactions",
        "workshop",
        "to",
        "with",
    ];
    let stopwords: HashSet<&str> = stopwords.into_iter().collect();
    let mut acronym = String::new();
    let tokens = tokenize_words(&clean_publication_name(name));

    if tokens.len() == 1 {
        let token = &tokens[0];
        if token.len() <= 10 {
            return token.to_ascii_uppercase();
        }
    }

    for token in tokens {
        let lower = token.to_ascii_lowercase();
        if stopwords.contains(lower.as_str()) {
            continue;
        }

        if let Some(initial) = token
            .chars()
            .find(|character| character.is_ascii_alphanumeric())
        {
            acronym.push(initial.to_ascii_uppercase());
        }
    }

    if acronym.len() > 10 {
        acronym.truncate(10);
    }

    acronym
}

fn extend_with_alias_keys(
    mut canonical_keys: Vec<String>,
    token: &str,
    expanded_value: &str,
) -> Vec<String> {
    let token_key = canonicalize_publication_name(token);
    if !token_key.is_empty() {
        canonical_keys.push(token_key);
    }

    let organizations = leading_organization_tokens(expanded_value);
    for index in 1..=organizations.len() {
        let alias = format!("{} {}", organizations[..index].join(" "), token);
        let alias_key = canonicalize_publication_name(&alias);
        if !alias_key.is_empty() {
            canonical_keys.push(alias_key);
        }
    }

    canonical_keys.sort();
    canonical_keys.dedup();
    canonical_keys
}

fn leading_organization_tokens(expanded_value: &str) -> Vec<String> {
    let mut organizations = Vec::new();

    for token in tokenize_words(expanded_value) {
        let is_org = token.len() >= 2
            && token
                .chars()
                .all(|character| character.is_ascii_uppercase());
        if !is_org {
            break;
        }
        organizations.push(token);
    }

    organizations
}

fn looks_like_abbreviated_publication_name(name: &str) -> bool {
    let tokens = tokenize_words(name);
    if tokens.is_empty() || tokens.len() > 4 {
        return false;
    }

    let descriptive_words = [
        "conference",
        "journal",
        "symposium",
        "workshop",
        "transactions",
        "review",
        "letters",
        "magazine",
    ];
    let descriptive_words: HashSet<&str> = descriptive_words.into_iter().collect();
    if tokens
        .iter()
        .any(|token| descriptive_words.contains(token.to_ascii_lowercase().as_str()))
    {
        return false;
    }

    tokens.iter().all(|token| token_looks_abbreviated(token))
}

fn token_looks_abbreviated(token: &str) -> bool {
    if token.len() > 10 {
        return false;
    }

    let has_uppercase = token
        .chars()
        .any(|character| character.is_ascii_uppercase());
    let has_lowercase = token
        .chars()
        .any(|character| character.is_ascii_lowercase());

    token
        .chars()
        .all(|character| character.is_ascii_alphanumeric())
        && ((has_uppercase && !has_lowercase)
            || (has_uppercase
                && has_lowercase
                && token.chars().next().is_some_and(|c| c.is_ascii_uppercase())))
}

fn tokenize_words(input: &str) -> Vec<String> {
    input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn clean_publication_name(raw: &str) -> String {
    let mut cleaned = normalize_unicode_dashes(&strip_outer_wrapping(raw).replace('\n', " "));
    cleaned = cleaned.replace("\\&", "and");
    cleaned = cleaned.replace('&', "and");
    cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    cleaned = remove_proceedings_prefix(&cleaned);
    cleaned = remove_leading_ordinals(&cleaned);
    cleaned = cleaned
        .trim_matches(|character: char| character == ',' || character == '.')
        .trim()
        .to_string();
    cleaned
}

fn remove_proceedings_prefix(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let prefixes = [
        "proceedings of the ",
        "proceedings of ",
        "proc. of the ",
        "proc. of ",
        "in proceedings of the ",
        "in proceedings of ",
    ];

    for prefix in prefixes {
        if lower.starts_with(prefix) {
            return value[prefix.len()..].trim().to_string();
        }
    }

    value.to_string()
}

fn remove_leading_ordinals(value: &str) -> String {
    let mut remaining = value.trim();
    loop {
        let mut parts = remaining.splitn(2, ' ');
        let first = parts.next().unwrap_or_default();
        let rest = parts.next().unwrap_or_default();
        let first_lower = first.to_ascii_lowercase();

        let is_year = first.len() == 4 && first.chars().all(|character| character.is_ascii_digit());
        let is_ordinal = first_lower.ends_with("st")
            || first_lower.ends_with("nd")
            || first_lower.ends_with("rd")
            || first_lower.ends_with("th");

        let numeric_prefix =
            first_lower.trim_end_matches(|character: char| character.is_ascii_alphabetic());
        if (is_year || is_ordinal)
            && numeric_prefix
                .chars()
                .all(|character| character.is_ascii_digit())
            && !rest.is_empty()
        {
            remaining = rest.trim();
            continue;
        }
        break;
    }

    remaining.to_string()
}

fn build_publication_lookup_keys(raw: &str) -> Vec<String> {
    let base = clean_publication_name(raw);
    let mut variants = vec![canonicalize_publication_name(&base)];
    let mut stripped = base.clone();

    while let Some(next) = remove_trailing_abbreviation_parenthetical(&stripped) {
        let canonical = canonicalize_publication_name(&next);
        if canonical.is_empty() || variants.contains(&canonical) {
            break;
        }
        variants.push(canonical);
        stripped = next;
    }

    variants.retain(|value| !value.is_empty());
    variants
}

fn remove_trailing_abbreviation_parenthetical(input: &str) -> Option<String> {
    let trimmed = input.trim_end();
    if !trimmed.ends_with(')') {
        return None;
    }

    let open_index = trimmed.rfind('(')?;
    let suffix = trimmed[open_index + 1..trimmed.len() - 1].trim();
    if !parenthetical_looks_like_abbreviation(suffix) {
        return None;
    }

    Some(trimmed[..open_index].trim_end().to_string())
}

fn parenthetical_looks_like_abbreviation(value: &str) -> bool {
    if value.is_empty() || value.contains(char::is_whitespace) || value.len() > 15 {
        return false;
    }

    let uppercase_count = value
        .chars()
        .filter(|character| character.is_ascii_uppercase())
        .count();
    let has_digit = value.chars().any(|character| character.is_ascii_digit());
    let has_symbol = value.chars().any(|character| {
        !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
    });

    !has_symbol && (uppercase_count >= 2 || has_digit)
}

fn canonicalize_publication_name(value: &str) -> String {
    let lowercase = value.to_ascii_lowercase();
    let mut canonical = String::new();

    for character in lowercase.chars() {
        if character.is_ascii_alphanumeric() {
            canonical.push(character);
        } else {
            canonical.push(' ');
        }
    }

    canonical.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tokenize_publication_similarity_tokens(value: &str) -> HashSet<String> {
    tokenize_words(value)
        .into_iter()
        .map(|token| token.to_ascii_lowercase())
        .filter(|token| token.len() > 1)
        .collect()
}

fn similarity_score(
    target_canonical: &str,
    target_tokens: &HashSet<String>,
    candidate_canonical: &str,
    candidate_tokens: &HashSet<String>,
) -> u16 {
    if target_canonical == candidate_canonical {
        return 100;
    }

    let shared_tokens = target_tokens.intersection(candidate_tokens).count() as f32;
    let union_tokens = target_tokens.union(candidate_tokens).count() as f32;
    let token_score = if union_tokens > 0.0 {
        shared_tokens / union_tokens
    } else {
        0.0
    };

    let string_score = normalized_string_overlap(target_canonical, candidate_canonical);
    ((token_score * 0.65 + string_score * 0.35) * 100.0).round() as u16
}

fn normalized_string_overlap(left: &str, right: &str) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let left_tokens = left.split_whitespace().collect::<Vec<_>>();
    let right_tokens = right.split_whitespace().collect::<Vec<_>>();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }

    let shared_prefix = left_tokens
        .iter()
        .zip(right_tokens.iter())
        .take_while(|(left_token, right_token)| left_token == right_token)
        .count() as f32;
    let shared_substring = longest_common_substring(left, right) as f32;
    let prefix_ratio = shared_prefix / left_tokens.len().max(right_tokens.len()) as f32;
    let substring_ratio = shared_substring / left.len().max(right.len()) as f32;
    prefix_ratio.max(substring_ratio)
}

fn longest_common_substring(left: &str, right: &str) -> usize {
    let left_chars = left.chars().collect::<Vec<_>>();
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut lengths = vec![0usize; right_chars.len() + 1];
    let mut best = 0usize;

    for left_char in left_chars {
        let mut next = vec![0usize; right_chars.len() + 1];
        for (index, right_char) in right_chars.iter().enumerate() {
            if left_char == *right_char {
                next[index + 1] = lengths[index] + 1;
                best = best.max(next[index + 1]);
            }
        }
        lengths = next;
    }

    best
}

fn normalize_year(raw: &str) -> String {
    strip_outer_wrapping(raw)
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

fn generate_unique_key(
    authors: &[String],
    year: &str,
    generated_keys: &mut HashMap<String, usize>,
) -> String {
    let key_year = key_year_fragment(year);
    let first = author_key_fragment(authors.first().map(String::as_str).unwrap_or_default());
    let second = if authors.len() >= 2 {
        author_key_fragment(authors[1].as_str())
    } else {
        String::new()
    };
    let base = format!("{first}{second}{key_year}");

    let entry = generated_keys.entry(base.clone()).or_insert(0);
    *entry += 1;

    if *entry == 1 {
        base
    } else {
        format!("{base}_{}", *entry)
    }
}

fn key_year_fragment(year: &str) -> String {
    let digits = year
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();

    if digits.len() >= 2 {
        digits[digits.len() - 2..].to_string()
    } else {
        digits
    }
}

fn author_key_fragment(author: &str) -> String {
    let surname = extract_surname(author);
    let letters = transliterate_key_text(&surname)
        .chars()
        .filter(|character| character.is_ascii_alphabetic())
        .collect::<String>();
    let mut iter = letters.chars();
    let first = iter.next().unwrap_or('X').to_ascii_uppercase();
    let second = iter.next().unwrap_or('x').to_ascii_lowercase();
    format!("{first}{second}")
}

fn transliterate_key_text(input: &str) -> String {
    let mut output = String::new();

    for character in input.chars() {
        match character {
            'ä' => output.push_str("ae"),
            'Ä' => output.push_str("Ae"),
            'ö' => output.push_str("oe"),
            'Ö' => output.push_str("Oe"),
            'ü' => output.push_str("ue"),
            'Ü' => output.push_str("Ue"),
            'ß' => output.push_str("ss"),
            'à' | 'á' | 'â' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' | 'ǎ' => output.push('a'),
            'À' | 'Á' | 'Â' | 'Ã' | 'Å' | 'Ā' | 'Ă' | 'Ą' | 'Ǎ' => output.push('A'),
            'æ' => output.push_str("ae"),
            'Æ' => output.push_str("Ae"),
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => output.push('c'),
            'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => output.push('C'),
            'ď' | 'đ' => output.push('d'),
            'Ď' | 'Đ' => output.push('D'),
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => output.push('e'),
            'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => output.push('E'),
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => output.push('i'),
            'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' => output.push('I'),
            'ł' => output.push('l'),
            'Ł' => output.push('L'),
            'ñ' | 'ń' | 'ņ' | 'ň' => output.push('n'),
            'Ñ' | 'Ń' | 'Ņ' | 'Ň' => output.push('N'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'ǒ' => output.push('o'),
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' | 'Ǒ' => output.push('O'),
            'œ' => output.push_str("oe"),
            'Œ' => output.push_str("Oe"),
            'ŕ' | 'ŗ' | 'ř' => output.push('r'),
            'Ŕ' | 'Ŗ' | 'Ř' => output.push('R'),
            'ś' | 'ŝ' | 'ş' | 'š' => output.push('s'),
            'Ś' | 'Ŝ' | 'Ş' | 'Š' => output.push('S'),
            'ţ' | 'ť' | 'ŧ' => output.push('t'),
            'Ţ' | 'Ť' | 'Ŧ' => output.push('T'),
            'ù' | 'ú' | 'û' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'ǔ' => output.push('u'),
            'Ù' | 'Ú' | 'Û' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' | 'Ǔ' => output.push('U'),
            'ý' | 'ÿ' | 'ŷ' => output.push('y'),
            'Ý' | 'Ÿ' | 'Ŷ' => output.push('Y'),
            'ź' | 'ż' | 'ž' => output.push('z'),
            'Ź' | 'Ż' | 'Ž' => output.push('Z'),
            other if other.is_ascii_alphabetic() => output.push(other),
            _ => {}
        }
    }

    output
}

fn extract_surname(author: &str) -> String {
    let normalized_author = strip_outer_wrapping(author);
    let author = normalized_author.trim();
    if let Some((last, _)) = author.split_once(',') {
        return last.trim().to_string();
    }

    let parts = author.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        return String::new();
    }
    if parts.len() == 1 {
        return parts[0].to_string();
    }

    let mut surname_parts = vec![parts[parts.len() - 1]];
    for part in parts[..parts.len() - 1].iter().rev() {
        if part.chars().all(|character| character.is_lowercase()) {
            surname_parts.push(part);
        } else {
            break;
        }
    }
    surname_parts.reverse();
    surname_parts.join(" ")
}

fn split_authors(raw: &str) -> Vec<String> {
    let value = strip_outer_wrapping(raw).replace('\n', " ");
    let bytes = value.as_bytes();
    let mut authors = Vec::new();
    let mut last = 0usize;
    let mut brace_depth = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'{' => brace_depth += 1,
            b'}' if brace_depth > 0 => brace_depth -= 1,
            b'a' | b'A' if brace_depth == 0 => {
                if value[index..].to_ascii_lowercase().starts_with("and") {
                    let before = if index == 0 { b' ' } else { bytes[index - 1] };
                    let after_index = index + 3;
                    let after = if after_index >= bytes.len() {
                        b' '
                    } else {
                        bytes[after_index]
                    };
                    if before.is_ascii_whitespace() && after.is_ascii_whitespace() {
                        let author = value[last..index].trim();
                        if !author.is_empty() {
                            authors.push(author.to_string());
                        }
                        last = after_index;
                        index = after_index;
                        continue;
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }

    let tail = value[last..].trim();
    if !tail.is_empty() {
        authors.push(tail.to_string());
    }

    authors
        .into_iter()
        .map(|author| {
            normalize_unicode_dashes(&author.split_whitespace().collect::<Vec<_>>().join(" "))
        })
        .collect()
}

/// Rewrites Unicode dashes that LaTeX cannot typeset directly into their
/// BibTeX equivalent. Every other non-ASCII character is deliberately left
/// untouched so that accented names survive formatting verbatim.
fn normalize_unicode_dashes(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            // en dash, em dash, horizontal bar, figure dash
            '\u{2013}' | '\u{2014}' | '\u{2015}' | '\u{2012}' => normalized.push_str("--"),
            // non-breaking and other exotic hyphens collapse to a plain hyphen
            '\u{2010}' | '\u{2011}' | '\u{2212}' => normalized.push('-'),
            _ => normalized.push(character),
        }
    }
    normalized
}

fn normalize_title(raw: &str) -> String {
    let stripped = normalize_unicode_dashes(&strip_outer_wrapping(raw));
    let words = stripped.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return String::new();
    }

    let stopwords = [
        "a", "an", "and", "as", "at", "by", "for", "from", "in", "into", "nor", "of", "on", "onto",
        "or", "per", "the", "to", "via", "with",
    ];
    let stopwords: HashSet<&str> = stopwords.into_iter().collect();

    let mut normalized_words = Vec::new();
    let mut previous_ended_subtitle = false;
    for (index, word) in words.iter().enumerate() {
        let is_last = index + 1 == words.len();
        let force_capitalize = index == 0 || is_last || previous_ended_subtitle;
        normalized_words.push(normalize_title_token(word, force_capitalize, &stopwords));
        previous_ended_subtitle = word.ends_with(':');
    }
    normalized_words.join(" ")
}

fn normalize_rfc_title(number: &str, raw: &str) -> String {
    let normalized_title = normalize_title(raw);
    strip_existing_rfc_title_prefix(&normalized_title, number)
}

fn strip_existing_rfc_title_prefix(title: &str, number: &str) -> String {
    let trimmed = title.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("rfc") {
        return trimmed.to_string();
    }

    let mut index = 3usize;
    let characters = trimmed.chars().collect::<Vec<_>>();
    while index < characters.len() && characters[index].is_whitespace() {
        index += 1;
    }

    let number_start = index;
    while index < characters.len() && characters[index].is_ascii_digit() {
        index += 1;
    }

    if number_start == index {
        return trimmed.to_string();
    }

    let candidate_number = characters[number_start..index].iter().collect::<String>();
    if candidate_number != number {
        return trimmed.to_string();
    }

    while index < characters.len()
        && (characters[index].is_whitespace()
            || matches!(characters[index], ':' | '-' | '–' | '—' | '.'))
    {
        index += 1;
    }

    let remainder = characters[index..]
        .iter()
        .collect::<String>()
        .trim()
        .to_string();
    if remainder.is_empty() {
        trimmed.to_string()
    } else {
        remainder
    }
}

fn normalize_title_token(word: &str, force_capitalize: bool, stopwords: &HashSet<&str>) -> String {
    let segments = word.split('-').collect::<Vec<_>>();
    let normalized_segments = segments
        .into_iter()
        .map(|segment| normalize_title_segment(segment, force_capitalize, stopwords))
        .collect::<Vec<_>>();
    normalized_segments.join("-")
}

fn normalize_title_segment(
    segment: &str,
    force_capitalize: bool,
    stopwords: &HashSet<&str>,
) -> String {
    let characters = segment.chars().collect::<Vec<_>>();
    let start = characters
        .iter()
        .position(|character| character.is_alphanumeric())
        .unwrap_or(0);
    let end = characters
        .iter()
        .rposition(|character| character.is_alphanumeric())
        .map(|index| index + 1)
        .unwrap_or(characters.len());

    let prefix = characters[..start].iter().collect::<String>();
    let core = characters[start..end].iter().collect::<String>();
    let suffix = characters[end..].iter().collect::<String>();

    if core.is_empty() {
        return segment.to_string();
    }

    let normalized_core = if core
        .chars()
        .filter(|character| character.is_ascii_uppercase())
        .count()
        >= 2
    {
        core
    } else {
        let lower = core.to_ascii_lowercase();
        if !force_capitalize && stopwords.contains(lower.as_str()) {
            lower
        } else {
            capitalize_first_alphanumeric(&lower)
        }
    };

    format!("{prefix}{normalized_core}{suffix}")
}

fn capitalize_first_alphanumeric(input: &str) -> String {
    let mut capitalized = String::with_capacity(input.len());
    let mut changed = false;

    for character in input.chars() {
        if !changed && character.is_alphanumeric() {
            // `to_uppercase` is Unicode-aware, so accented initials such as
            // '\u{fc}' are capitalised too. Multi-character expansions (for
            // example '\u{df}' -> "SS") are skipped to keep the word intact.
            let mut uppercased = character.to_uppercase();
            match (uppercased.next(), uppercased.next()) {
                (Some(single), None) => capitalized.push(single),
                _ => capitalized.push(character),
            }
            changed = true;
        } else {
            capitalized.push(character);
        }
    }

    capitalized
}

fn normalize_month(raw: &str) -> String {
    let cleaned = strip_outer_wrapping(raw)
        .trim()
        .trim_matches('.')
        .to_ascii_lowercase();
    let month = match cleaned.as_str() {
        "january" | "jan" => "jan",
        "february" | "feb" => "feb",
        "march" | "mar" => "mar",
        "april" | "apr" => "apr",
        "may" => "may",
        "june" | "jun" => "jun",
        "july" | "jul" => "jul",
        "august" | "aug" => "aug",
        "september" | "sep" | "sept" => "sep",
        "october" | "oct" => "oct",
        "november" | "nov" => "nov",
        "december" | "dec" => "dec",
        other => other,
    };
    month.to_string()
}

fn normalize_pages(
    raw_pages: &str,
    numpages: Option<&str>,
    drop_leading_one_without_numpages: bool,
) -> Option<String> {
    let cleaned = strip_outer_wrapping(raw_pages)
        .replace(
            ['\u{2010}', '\u{2011}', '\u{2012}', '\u{2013}', '\u{2014}', '\u{2015}', '\u{2212}'],
            "-",
        )
        .replace(' ', "");

    let split = cleaned.split_once("--").or_else(|| cleaned.split_once('-'));

    if let Some((start, end)) = split {
        if let (Ok(start_page), Ok(end_page)) = (start.parse::<u32>(), end.parse::<u32>()) {
            if start_page == 1 {
                if let Some(total_pages) = numpages.and_then(|value| value.parse::<u32>().ok()) {
                    if end_page == total_pages {
                        return None;
                    }
                } else if drop_leading_one_without_numpages {
                    return None;
                }
            }
            return Some(format!("{start_page}--{end_page}"));
        }
    }

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.replace('-', "--"))
    }
}

pub fn strip_outer_wrapping(raw: &str) -> String {
    let mut current = raw.trim().to_string();
    loop {
        let bytes = current.as_bytes();
        if bytes.len() < 2 {
            return current;
        }

        let wrapped_in_braces = bytes.first() == Some(&b'{')
            && bytes.last() == Some(&b'}')
            && is_single_wrapped_expression(&current, b'{', b'}');
        let wrapped_in_quotes = bytes.first() == Some(&b'"')
            && bytes.last() == Some(&b'"')
            && is_single_wrapped_expression(&current, b'"', b'"');

        if wrapped_in_braces || wrapped_in_quotes {
            current = current[1..current.len() - 1].trim().to_string();
            continue;
        }

        return current;
    }
}

fn is_single_wrapped_expression(input: &str, open: u8, close: u8) -> bool {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&open) || bytes.last() != Some(&close) {
        return false;
    }

    let mut depth = 0usize;
    let mut in_quotes = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().copied().enumerate() {
        if open == b'"' {
            if index == 0 || index + 1 == bytes.len() {
                continue;
            }
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' && !is_escaped(bytes, index) {
                in_quotes = !in_quotes;
            }
            continue;
        }

        if in_quotes {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_quotes = false;
            }
            continue;
        }

        match byte {
            b'"' if !is_escaped(bytes, index) => in_quotes = true,
            b if b == open => depth += 1,
            b if b == close => {
                depth -= 1;
                if depth == 0 && index + 1 != bytes.len() {
                    return false;
                }
            }
            _ => {}
        }
    }

    depth == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_entries_and_preserves_raw_text() {
        let input = "@comment{keep this, exactly}\n\n@misc{skip,\n  note = {Leave me alone}\n}\n";
        let entries = parse_bibtex_document(input).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].raw_text, "@comment{keep this, exactly}");
        assert_eq!(
            entries[1].raw_text,
            "@misc{skip,\n  note = {Leave me alone}\n}"
        );
    }

    #[test]
    fn normalizes_keys_and_title() {
        let authors = vec![
            "Kannan, Pravein Govindan".to_string(),
            "Joshi, Raj".to_string(),
        ];
        let mut generated = HashMap::new();
        assert_eq!(
            generate_unique_key(&authors, "2019", &mut generated),
            "KaJo19"
        );
        let umlaut_authors = vec!["Lindner, Steffen".to_string(), "Häberle, Marco".to_string()];
        assert_eq!(
            generate_unique_key(&umlaut_authors, "2023", &mut HashMap::new()),
            "LiHa23"
        );
        assert_eq!(
            normalize_title(
                "precise time-synchronization in the data-plane using programmable switching ASICs"
            ),
            "Precise Time-Synchronization in the Data-Plane Using Programmable Switching ASICs"
        );
        assert_eq!(
            normalize_title("p4: programming protocol-independent packet processors"),
            "P4: Programming Protocol-Independent Packet Processors"
        );
    }

    #[test]
    fn normalizes_unicode_dashes_without_touching_other_unicode() {
        assert_eq!(
            normalize_title("network slicing \u{2013} a survey"),
            "Network Slicing -- a Survey"
        );
        assert_eq!(
            normalize_title("p4 \u{2014} programming switches"),
            "P4 -- Programming Switches"
        );
        assert_eq!(
            normalize_title("m\u{fc}nchen und t\u{fc}bingen: \u{fc}bertragung"),
            "M\u{fc}nchen Und T\u{fc}bingen: \u{dc}bertragung"
        );
        assert_eq!(
            split_authors("H\u{e4}berle, Marco and Well\u{2013}Meyer, Jan"),
            vec![
                "H\u{e4}berle, Marco".to_string(),
                "Well--Meyer, Jan".to_string()
            ]
        );
        assert_eq!(normalize_pages("{1\u{2013}10}", None, false), Some("1--10".to_string()));
    }

    #[test]
    fn formats_entries_whose_title_contains_an_en_dash() {
        let input = "@inproceedings{HoVe19,\n  author = {Hoffmann, Anna and Vetter, Bob},\n  title = {Network Slicing \u{2013} A Survey},\n  booktitle = {IEEE International Conference on Communications (ICC)},\n  year = {2019},\n  pages = {12\u{2013}20}\n}\n";
        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source("").unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: false,
                use_conferences_bib: false,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .expect("formatting must not fail on unicode dashes");
        let output = &result.entries[0].output_text;
        assert!(
            output.contains("title = {{Network Slicing -- a Survey}}"),
            "unexpected output: {output}"
        );
        assert!(output.contains("pages = {12--20}"), "unexpected output: {output}");
    }

    #[test]
    fn normalizes_title_segments_made_only_of_non_ascii_characters() {
        assert_eq!(normalize_title("a \u{2013} b"), "A -- B");
        assert_eq!(normalize_title("\u{201c}\u{201d}"), "\u{201c}\u{201d}");
    }

    #[test]
    fn drops_trivial_page_ranges() {
        assert_eq!(normalize_pages("{1-9}", Some("9"), true), None);
        assert_eq!(normalize_pages("{1-8}", None, true), None);
        assert_eq!(
            normalize_pages("{1-8}", None, false),
            Some("1--8".to_string())
        );
        assert_eq!(
            normalize_pages("{8-20}", Some("13"), true),
            Some("8--20".to_string())
        );
    }

    #[test]
    fn creates_unique_abbreviation_tokens() {
        let source = "@STRING{SOSR = {{ACM Symposium on SDN Research (SOSR)}}}";
        let definitions = DefinitionCollection::from_source(source).unwrap();
        let created = create_definition("ACM Symposium on SDN Research", &definitions);
        assert_eq!(created.token, "ASR");
        assert_eq!(created.raw_value, "{{ACM Symposium on SDN Research (ASR)}}");
    }

    #[test]
    fn suggests_similar_existing_venues_for_new_abbreviations() {
        let source = r#"
@STRING{OJCOMS = {{IEEE Open Journal of the Communications Society (OJCOMS)}}}
@STRING{NETSYS = {{International Conference on Networked Systems (NetSys)}}}
"#;
        let definitions = DefinitionCollection::from_source(source).unwrap();
        let suggestions =
            definitions.similar_by_name("IEEE Open Journal of Communications Society", 3);

        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].token, "OJCOMS");
        assert!(suggestions[0].score >= 45);
    }

    #[test]
    fn formatter_attaches_venue_suggestions_to_new_proposals() {
        let input = r#"
@article{oj,
  author = {Ihle, Fabian and Menth, Michael},
  title = {MPLS Network Actions},
  year = {2025},
  journal = {IEEE Open Journal of Communications Society}
}
"#;
        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source(
            "@STRING{OJCOMS = {{IEEE Open Journal of the Communications Society (OJCOMS)}}}",
        )
        .unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert_eq!(result.added_definitions.len(), 1);
        assert_eq!(result.added_definitions[0].suggestions[0].token, "OJCOMS");
    }

    #[test]
    fn disabling_conferences_bib_disables_venue_suggestions() {
        let input = r#"
@article{oj,
  author = {Ihle, Fabian and Menth, Michael},
  title = {MPLS Network Actions},
  year = {2025},
  journal = {IEEE Open Journal of Communications Society}
}
"#;
        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source(
            "@STRING{OJCOMS = {{IEEE Open Journal of the Communications Society (OJCOMS)}}}",
        )
        .unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: false,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert!(result.added_definitions.is_empty());
        assert!(
            result
                .output
                .contains("journal = {IEEE Open Journal of Communications Society}")
        );
    }

    #[test]
    fn formats_mixed_batch_and_keeps_unsupported_entries() {
        let input = r#"
@inproceedings{foo,
  author = {Doe, Jane and Smith, John},
  title = {example title},
  year = {2024},
  booktitle = {Proceedings of the 19th ACM Symposium on SDN Research (SOSR)},
  pages = {1-10},
  month = {April},
  abstract = {remove}
}

@misc{untouched,
  note = {Preserve me}
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source(
            "@STRING{SOSR = {{ACM Symposium on SDN Research (SOSR)}}}",
        )
        .unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert_eq!(result.entry_count, 2);
        assert_eq!(result.formatted_count, 1);
        assert_eq!(result.untouched_count, 1);
        assert!(
            result
                .output
                .contains("@misc{untouched,\n  note = {Preserve me}\n}")
        );
        assert_eq!(result.entries[1].status, EntryStatus::Untouched);
        assert!(
            result.entries[0]
                .removed_fields
                .contains(&"abstract".to_string())
        );
        assert!(
            result.entries[0]
                .normalized_fields
                .contains(&"title".to_string())
        );
    }

    #[test]
    fn formats_rfc_entries_as_techreports_and_removes_export_noise() {
        let input = r#"
@misc{rfc1034,
    series =    {Request for Comments},
    number =    1034,
    howpublished =  {RFC 1034},
    publisher = {RFC Editor},
    doi =       {10.17487/RFC1034},
    url =       {https://www.rfc-editor.org/info/rfc1034},
    author =    {Mockapetris, Paul},
    title =     {{Domain names - concepts and facilities}},
    pagetotal = 55,
    year =      1987,
    month =     nov,
    abstract =  {remove me},
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source("").unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert_eq!(result.entry_count, 1);
        assert_eq!(result.formatted_count, 1);
        assert_eq!(
            result.output,
            r#"@techreport{rfc1034,
  series      = {Request for Comments},
  number      = 1034,
  institution = {IETF},
  type        = {RFC},
  author      = {Mockapetris, Paul},
  title       = {{Domain Names - Concepts and Facilities}},
  pagetotal   = 55,
  year        = 1987,
  month       = nov
}"#
        );
        assert_eq!(result.entries[0].entry_type, "techreport");
        assert!(!result.output.contains("howpublished"));
        assert!(!result.output.contains("publisher"));
        assert!(!result.output.contains("doi"));
        assert!(!result.output.contains("url"));
        assert!(!result.output.contains("abstract"));
        assert!(
            result.entries[0]
                .removed_fields
                .contains(&"howpublished".to_string())
        );
        assert!(
            result.entries[0]
                .removed_fields
                .contains(&"publisher".to_string())
        );
    }

    #[test]
    fn removes_existing_rfc_title_prefix_from_techreports() {
        let input = r#"
@techreport{rfc9023,
    series = {Request for Comments},
    number = {9023},
    institution = {IETF},
    type = {RFC},
    author = {Varga, Balazs},
    title = {{RFC 9023: Deterministic Networking Data Plane}},
    pagetotal = 10,
    year = 2021,
    month = jun,
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source("").unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert!(
            result
                .output
                .contains("title       = {{Deterministic Networking Data Plane}}")
        );
        assert!(!result.output.contains("RFC9023:"));
    }

    #[test]
    fn keeps_doi_only_when_enabled() {
        let input = r#"
@article{example,
  author = {Doe, Jane and Smith, John},
  title = {example title},
  year = {2024},
  journal = {Example Journal},
  doi = {10.1000/EXAMPLE}
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source("").unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: false,
                use_conferences_bib: false,
                rewrite_reference_key: true,
                keep_doi: true,
            },
        )
        .unwrap();

        assert!(result.output.contains("doi = {10.1000/EXAMPLE}"));
        assert!(
            !result.entries[0]
                .removed_fields
                .contains(&"doi".to_string())
        );
    }

    #[test]
    fn keeps_rfc_doi_when_enabled() {
        let input = r#"
@misc{rfc5884,
  series = {Request for Comments},
  number = 5884,
  author = {Rahul Aggarwal and Kireeti Kompella and Thomas Nadeau and George Swallow},
  title = {Bidirectional Forwarding Detection (BFD) for MPLS Label Switched Paths (LSPs)},
  pagetotal = 12,
  year = 2010,
  month = jun,
  doi = {10.17487/RFC5884}
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source("").unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: false,
                use_conferences_bib: false,
                rewrite_reference_key: true,
                keep_doi: true,
            },
        )
        .unwrap();

        assert_eq!(
            result.output,
            r#"@techreport{rfc5884,
  series      = {Request for Comments},
  number      = 5884,
  institution = {IETF},
  type        = {RFC},
  author      = {Rahul Aggarwal and Kireeti Kompella and Thomas Nadeau and George Swallow},
  title       = {{Bidirectional Forwarding Detection (BFD) for MPLS Label Switched Paths (LSPs)}},
  pagetotal   = 12,
  year        = 2010,
  month       = jun,
  doi         = {10.17487/RFC5884}
}"#
        );
        assert!(
            !result.entries[0]
                .removed_fields
                .contains(&"doi".to_string())
        );
    }

    #[test]
    fn formats_journal_less_articles_as_misc() {
        let input = r#"
@article{https://doi.org/10.25972/opus-38896,
  doi = {10.25972/OPUS-38896},
  url = {https://opus.bibliothek.uni-wuerzburg.de/38896},
  author = {Ihle, Fabian and Fluechter, Moritz and Lindner, Steffen and Menth, Michael},
  keywords = {004 Datenverarbeitung; Informatik},
  language = {en},
  title = {UniCorn-P4: A Universal Control Plane and GUI for P4},
  publisher = {Universitaet Wuerzburg},
  year = {2025}
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source("").unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert!(result.output.contains("@misc{IhFl25,"));
        assert!(result.output.contains(
            "author = {Ihle, Fabian and Fluechter, Moritz and Lindner, Steffen and Menth, Michael}"
        ));
        assert!(
            result
                .output
                .contains("title = {{UniCorn-P4: A Universal Control Plane and GUI for P4}}")
        );
        assert!(result.output.contains("year = 2025"));
        assert!(!result.output.contains("doi ="));
        assert!(!result.output.contains("publisher ="));
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("formatted entry")
                    || warning.contains("Formatted entry"))
        );
    }

    #[test]
    fn formats_ieee_standards_without_authors() {
        let input = r#"
@ARTICLE{cb,
  journal={IEEE Std 802.1CB},
  title={{IEEE Standard for Local and Metropolitan Area Networks--Frame Replication and Elimination for Reliability}},
  year={2017},
  doi={10.1109/IEEESTD.2017.8091139}
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source("").unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert!(result.output.contains("@article{cb,"));
        assert!(result.output.contains("journal = {IEEE Std 802.1CB}"));
        assert!(result.output.contains("year = 2017"));
        assert!(!result.output.contains("doi ="));
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("standard-like"))
        );
    }

    #[test]
    fn skips_duplicate_inline_string_definitions_from_input() {
        let input = r#"
@STRING{SOSR = {{ACM Symposium on SDN Research (SOSR)}}}

@inproceedings{foo,
  author = {Doe, Jane and Smith, John},
  title = {example title},
  year = {2024},
  booktitle = {Proceedings of the 19th ACM Symposium on SDN Research (SOSR)}
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source(
            "@STRING{SOSR = {{ACM Symposium on SDN Research (SOSR)}}}",
        )
        .unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert_eq!(result.output.matches("@STRING{SOSR").count(), 1);
    }

    #[test]
    fn resolves_existing_string_tokens_directly() {
        let definitions = DefinitionCollection::from_source(
            "@STRING{CCR = {{ACM SIGCOMM Computer Communication Review}}}",
        )
        .unwrap();
        let matched = definitions.find_by_name("CCR").unwrap();
        assert_eq!(matched.token, "CCR");
    }

    #[test]
    fn resolves_existing_definitions_from_short_org_prefixed_aliases() {
        let definitions =
            DefinitionCollection::from_source("@STRING{NOMS = {{IEEE/IFIP Network Operations and Management Symposium (NOMS)}}}\n@STRING{TNSM = {{{IEEE} Transactions on Network and Service Management}}}")
                .unwrap();

        assert_eq!(definitions.find_by_name("IEEE NOMS").unwrap().token, "NOMS");
        assert_eq!(definitions.find_by_name("IEEE TNSM").unwrap().token, "TNSM");
    }

    #[test]
    fn resolves_existing_definitions_when_input_omits_trailing_abbreviation_suffix() {
        let definitions = DefinitionCollection::from_source(
            "@STRING{OJCOMS = {{IEEE Open Journal of the Communications Society (OJCOMS)}}}",
        )
        .unwrap();

        assert_eq!(
            definitions
                .find_by_name("IEEE Open Journal of the Communications Society")
                .unwrap()
                .token,
            "OJCOMS"
        );
    }

    #[test]
    fn does_not_strip_non_abbreviation_parenthetical_suffixes_for_matching() {
        let definitions = DefinitionCollection::from_source(
            "@STRING{FOOW = {{International Conference on Foo (Workshop)}}}",
        )
        .unwrap();

        assert!(
            definitions
                .find_by_name("International Conference on Foo")
                .is_none()
        );
    }

    #[test]
    fn keeps_unresolved_short_publication_aliases_literal() {
        let input = r#"
@article{foo,
  author = {Doe, Jane and Smith, John},
  title = {example title},
  year = {2024},
  journal = {KuVS NetSoft},
  pages = {8-10}
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source("").unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert!(result.output.contains("journal = {KuVS NetSoft}"));
        assert!(result.added_definitions.is_empty());
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("looks like an abbreviation"))
        );
    }

    #[test]
    fn keeps_publication_names_literal_when_conference_lookup_is_disabled() {
        let input = r#"
@article{foo,
  author = {Doe, Jane and Smith, John},
  title = {example title},
  year = {2024},
  journal = {IEEE Open Journal of the Communications Society},
  pages = {8-10}
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source(
            "@STRING{OJCOMS = {{IEEE Open Journal of the Communications Society (OJCOMS)}}}",
        )
        .unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: false,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert!(
            result
                .output
                .contains("journal = {IEEE Open Journal of the Communications Society}")
        );
        assert!(result.added_definitions.is_empty());
        assert!(result.output_definitions.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn keeps_original_reference_key_when_rewrite_is_disabled() {
        let input = r#"
@inproceedings{custom-key,
  author = {Doe, Jane and Smith, John},
  title = {example title},
  year = {2024},
  booktitle = {Proceedings of the 19th ACM Symposium on SDN Research (SOSR)}
}
"#;

        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions = DefinitionCollection::from_source(
            "@STRING{SOSR = {{ACM Symposium on SDN Research (SOSR)}}}",
        )
        .unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: false,
                keep_doi: false,
            },
        )
        .unwrap();

        assert!(result.output.contains("@inproceedings{custom-key,"));
        assert_eq!(result.entries[0].key, "custom-key");
        assert!(
            !result.entries[0]
                .normalized_fields
                .contains(&"key".to_string())
        );
    }

    #[test]
    fn formats_sample_literature_file_without_failing() {
        let input = include_str!("../../sample_literature.bib");
        let entries = parse_bibtex_document(input).unwrap();
        let mut definitions =
            DefinitionCollection::from_source(include_str!("../../conferences.bib")).unwrap();
        let result = format_entries(
            &entries,
            &mut definitions,
            &FormatOptions {
                include_string_definitions: true,
                use_conferences_bib: true,
                rewrite_reference_key: true,
                keep_doi: false,
            },
        )
        .unwrap();

        assert!(result.entry_count > 10);
        assert!(result.formatted_count > 0);
        assert!(result.untouched_count > 0);
        assert!(result.output.contains("@misc{MaCa22,"));
        assert!(result.output.contains("@inproceedings{KaJo19,"));
        assert!(result.output.contains("@article{cb,"));
        assert!(result.output.contains("@techreport{rfc9023,"));
        assert!(
            result
                .output
                .contains("series      = {Request for Comments}")
        );
        assert!(!result.output.contains("howpublished =  {RFC 8655}"));
    }
}
