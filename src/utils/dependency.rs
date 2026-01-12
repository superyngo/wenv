//! Dependency analysis for environment variables and aliases
//!
//! This module detects variable references (like $VAR or ${VAR}) in entry values
//! and performs topological sorting to ensure dependencies are defined before use.

use crate::model::Entry;
use regex::Regex;
use std::collections::{HashMap, HashSet, VecDeque};

/// Extract variable names referenced in a string
/// Matches: $VAR, ${VAR}, $VAR_NAME, etc.
pub fn extract_variable_refs(text: &str) -> HashSet<String> {
    let mut vars = HashSet::new();

    // Match $VAR or ${VAR} patterns
    // Pattern explanation:
    // \$\{([A-Za-z_][A-Za-z0-9_]*)\}  - matches ${VAR_NAME}
    // \$([A-Za-z_][A-Za-z0-9_]*)      - matches $VAR_NAME
    let re = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();

    for cap in re.captures_iter(text) {
        // Try group 1 (${VAR} format) first, then group 2 ($VAR format)
        if let Some(var) = cap.get(1).or_else(|| cap.get(2)) {
            vars.insert(var.as_str().to_string());
        }
    }

    vars
}

/// Topologically sort entries based on variable dependencies
///
/// Returns entries sorted so that:
/// 1. Variables are defined before they are referenced
/// 2. Within each dependency level, entries are sorted alphabetically
/// 3. Circular dependencies are detected and handled by preserving original order
///
/// # Arguments
/// * `entries` - Slice of Entry references to sort
/// * `sort_alphabetically` - Whether to sort alphabetically within dependency levels
///
/// # Returns
/// Vector of sorted entries
pub fn topological_sort<'a>(entries: &[&'a Entry], sort_alphabetically: bool) -> Vec<&'a Entry> {
    // Build dependency graph
    // Key: variable name, Value: entry that defines it
    let mut definitions: HashMap<String, &Entry> = HashMap::new();

    // Graph edges: dep_ptr -> entry_ptr means "dep must come before entry"
    // Key: entry pointer, Value: set of entries that THIS entry depends on
    let mut depends_on: HashMap<*const Entry, HashSet<*const Entry>> = HashMap::new();

    // Key: entry pointer, Value: count of entries that depend on THIS entry
    let mut in_degree: HashMap<*const Entry, usize> = HashMap::new();

    // First pass: record all definitions
    for entry in entries {
        let ptr = *entry as *const Entry;
        definitions.insert(entry.name.clone(), entry);
        in_degree.insert(ptr, 0);
        depends_on.insert(ptr, HashSet::new());
    }

    // Second pass: build dependency edges
    // If AAA depends on ZZZ, we add edge ZZZ -> AAA
    for entry in entries {
        let entry_ptr = *entry as *const Entry;
        let refs = extract_variable_refs(&entry.value);

        for var_name in refs {
            // Check if this variable is defined by another entry in our set
            if let Some(dep_entry) = definitions.get(&var_name) {
                let dep_ptr = *dep_entry as *const Entry;

                // Don't create self-dependency
                if dep_ptr != entry_ptr {
                    // entry depends on dep_entry
                    // So we need edge: dep_entry -> entry
                    depends_on.get_mut(&entry_ptr).unwrap().insert(dep_ptr);
                    // entry has incoming edge, increase its in_degree
                    *in_degree.get_mut(&entry_ptr).unwrap() += 1;
                }
            }
        }
    }

    // Kahn's algorithm for topological sort
    let mut result = Vec::new();
    let mut queue: VecDeque<&Entry> = VecDeque::new();

    // Start with entries that have no dependencies (in_degree == 0)
    let mut entries_by_degree: Vec<_> = entries.to_vec();
    if sort_alphabetically {
        // Sort alphabetically first, so entries with same in_degree maintain alphabetical order
        entries_by_degree.sort_by(|a, b| a.name.cmp(&b.name));
    }

    for entry in &entries_by_degree {
        let ptr = *entry as *const Entry;
        if in_degree[&ptr] == 0 {
            queue.push_back(entry);
        }
    }

    // Build reverse map: which entries depend on each entry?
    let mut depended_by: HashMap<*const Entry, Vec<*const Entry>> = HashMap::new();
    for entry in entries {
        depended_by.insert(*entry as *const Entry, Vec::new());
    }
    for entry in entries {
        let entry_ptr = *entry as *const Entry;
        for &dep_ptr in &depends_on[&entry_ptr] {
            depended_by.get_mut(&dep_ptr).unwrap().push(entry_ptr);
        }
    }

    while let Some(entry) = queue.pop_front() {
        result.push(entry);
        let entry_ptr = entry as *const Entry;

        // Collect entries that will become available
        let mut newly_available = Vec::new();

        // For each entry that depends on this one
        if let Some(dependent_ptrs) = depended_by.get(&entry_ptr) {
            for &dependent_ptr in dependent_ptrs {
                // Decrease in-degree of dependent
                let degree = in_degree.get_mut(&dependent_ptr).unwrap();
                *degree -= 1;

                if *degree == 0 {
                    // Find the entry with this pointer
                    let dependent_entry = entries
                        .iter()
                        .find(|e| std::ptr::eq(**e, dependent_ptr))
                        .unwrap();
                    newly_available.push(dependent_entry);
                }
            }
        }

        // Sort newly available entries alphabetically if needed, then add to queue
        if sort_alphabetically {
            newly_available.sort_by(|a, b| a.name.cmp(&b.name));
        }
        for entry in newly_available {
            queue.push_back(entry);
        }
    }

    // Check for circular dependencies
    if result.len() != entries.len() {
        // Circular dependency detected - fall back to original order
        eprintln!("Warning: Circular dependency detected in environment variables, preserving original order");
        return entries.to_vec();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EntryType;

    #[test]
    fn test_extract_variable_refs() {
        // Test $VAR format
        let refs = extract_variable_refs("$HOME/bin");
        assert!(refs.contains("HOME"));
        assert_eq!(refs.len(), 1);

        // Test ${VAR} format
        let refs = extract_variable_refs("${HOME}/bin:${PATH}");
        assert!(refs.contains("HOME"));
        assert!(refs.contains("PATH"));
        assert_eq!(refs.len(), 2);

        // Test mixed format
        let refs = extract_variable_refs("$HOME/bin:${PATH}");
        assert!(refs.contains("HOME"));
        assert!(refs.contains("PATH"));
        assert_eq!(refs.len(), 2);

        // Test no variables
        let refs = extract_variable_refs("/usr/local/bin");
        assert_eq!(refs.len(), 0);

        // Test variable with underscores and numbers
        let refs = extract_variable_refs("$MY_VAR_123");
        assert!(refs.contains("MY_VAR_123"));
    }

    #[test]
    fn test_topological_sort_simple() {
        let entries = vec![
            Entry::new(EntryType::EnvVar, "AAA".into(), "$ZZZ/bin".into()).with_line_number(1),
            Entry::new(EntryType::EnvVar, "ZZZ".into(), "/usr/local".into()).with_line_number(2),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let sorted = topological_sort(&entry_refs, true);

        // ZZZ should come before AAA (dependency order)
        assert_eq!(sorted[0].name, "ZZZ");
        assert_eq!(sorted[1].name, "AAA");
    }

    #[test]
    fn test_topological_sort_chain() {
        let entries = vec![
            Entry::new(EntryType::EnvVar, "PATH".into(), "$BIN:$PATH".into()).with_line_number(3),
            Entry::new(EntryType::EnvVar, "BIN".into(), "$BASE/bin".into()).with_line_number(2),
            Entry::new(EntryType::EnvVar, "BASE".into(), "/usr/local".into()).with_line_number(1),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let sorted = topological_sort(&entry_refs, false);

        // Order should be: BASE -> BIN -> PATH
        assert_eq!(sorted[0].name, "BASE");
        assert_eq!(sorted[1].name, "BIN");
        assert_eq!(sorted[2].name, "PATH");
    }

    #[test]
    fn test_topological_sort_no_dependencies() {
        let entries = vec![
            Entry::new(EntryType::EnvVar, "ZZZ".into(), "zzz".into()),
            Entry::new(EntryType::EnvVar, "AAA".into(), "aaa".into()),
            Entry::new(EntryType::EnvVar, "MMM".into(), "mmm".into()),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let sorted = topological_sort(&entry_refs, true);

        // Should be sorted alphabetically when no dependencies
        assert_eq!(sorted[0].name, "AAA");
        assert_eq!(sorted[1].name, "MMM");
        assert_eq!(sorted[2].name, "ZZZ");
    }

    #[test]
    fn test_topological_sort_alphabetical_within_level() {
        // Multiple variables depend on BASE, they should be alphabetical
        let entries = vec![
            Entry::new(EntryType::EnvVar, "BASE".into(), "/usr/local".into()),
            Entry::new(EntryType::EnvVar, "ZZZ_PATH".into(), "$BASE/zzz".into()),
            Entry::new(EntryType::EnvVar, "AAA_PATH".into(), "$BASE/aaa".into()),
            Entry::new(EntryType::EnvVar, "MMM_PATH".into(), "$BASE/mmm".into()),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let sorted = topological_sort(&entry_refs, true);

        // BASE should be first
        assert_eq!(sorted[0].name, "BASE");

        // The rest should be alphabetical (all same dependency level)
        assert_eq!(sorted[1].name, "AAA_PATH");
        assert_eq!(sorted[2].name, "MMM_PATH");
        assert_eq!(sorted[3].name, "ZZZ_PATH");
    }
}
