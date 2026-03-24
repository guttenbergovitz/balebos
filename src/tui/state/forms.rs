//! Form state management for Organization add/edit operations.

/// State for organization forms (add/edit person, company, brand, project)
#[derive(Debug, Clone)]
pub struct FormState {
    /// Current form field values (one string per field)
    pub fields: Vec<String>,

    /// Index of the currently active field (cursor position in form)
    pub field_index: usize,

    /// Index into org_select_matches for ID field autocomplete
    /// (e.g., selecting a Brand ID or Company ID)
    pub select_index: Option<usize>,

    /// Filtered matches for the current ID field
    /// Tuple of (Display Name, ID string)
    pub select_matches: Vec<(String, String)>,
}

impl FormState {
    /// Create a new form with the given number of fields
    pub fn new(field_count: usize) -> Self {
        Self {
            fields: vec![String::new(); field_count],
            field_index: 0,
            select_index: None,
            select_matches: Vec::new(),
        }
    }

    /// Move to the next field (if available)
    pub fn next_field(&mut self, total_fields: usize) -> bool {
        if self.field_index < total_fields - 1 {
            self.field_index += 1;
            self.reset_select();
            true
        } else {
            false
        }
    }

    /// Move to the previous field (if available)
    pub fn previous_field(&mut self) -> bool {
        if self.field_index > 0 {
            self.field_index -= 1;
            self.reset_select();
            true
        } else {
            false
        }
    }

    /// Get the current field value
    pub fn current_value(&self) -> &str {
        self.fields
            .get(self.field_index)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Get mutable reference to the current field
    fn current_field_mut(&mut self) -> Option<&mut String> {
        self.fields.get_mut(self.field_index)
    }

    /// Push a character to the current field
    pub fn push_char(&mut self, c: char) {
        if let Some(field) = self.current_field_mut() {
            field.push(c);
        }
    }

    /// Pop a character from the current field
    pub fn pop_char(&mut self) {
        if let Some(field) = self.current_field_mut() {
            field.pop();
        }
    }

    /// Clear all form fields
    pub fn clear(&mut self) {
        self.fields.iter_mut().for_each(|f| f.clear());
        self.field_index = 0;
        self.reset_select();
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.clear();
    }

    /// Navigate to next match in select list
    pub fn select_next(&mut self) {
        if !self.select_matches.is_empty() {
            let i = self.select_index.map(|i| i + 1).unwrap_or(0);
            if i >= self.select_matches.len() {
                self.select_index = Some(0);
            } else {
                self.select_index = Some(i);
            }
        }
    }

    /// Navigate to previous match in select list
    pub fn select_previous(&mut self) {
        if !self.select_matches.is_empty() {
            let i = self
                .select_index
                .map(|i| {
                    if i == 0 {
                        self.select_matches.len() - 1
                    } else {
                        i - 1
                    }
                })
                .unwrap_or(self.select_matches.len() - 1);
            self.select_index = Some(i);
        }
    }

    /// Get the currently selected match (if any)
    pub fn selected_match(&self) -> Option<&(String, String)> {
        self.select_index.and_then(|i| self.select_matches.get(i))
    }

    /// Set the matches for the current field's autocomplete
    pub fn set_select_matches(&mut self, matches: Vec<(String, String)>) {
        self.select_matches = matches;
        // Clamp select_index to new matches length
        if let Some(i) = self.select_index {
            if i >= self.select_matches.len() {
                self.select_index = if self.select_matches.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
    }

    /// Reset the select list (e.g., when moving to a non-ID field)
    pub fn reset_select(&mut self) {
        self.select_index = None;
        self.select_matches.clear();
    }

    /// Set value for a specific field
    pub fn set_field_value(&mut self, index: usize, value: String) {
        if index < self.fields.len() {
            self.fields[index] = value;
        }
    }

    /// Get value for a specific field
    pub fn get_field_value(&self, index: usize) -> Option<&str> {
        self.fields.get(index).map(|s| s.as_str())
    }

    /// Total number of fields
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
}

impl Default for FormState {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let form = FormState::new(3);
        assert_eq!(form.field_count(), 3);
        assert_eq!(form.field_index, 0);
        assert!(form.fields.iter().all(|f| f.is_empty()));
    }

    #[test]
    fn test_next_field() {
        let mut form = FormState::new(3);
        assert!(form.next_field(3));
        assert_eq!(form.field_index, 1);
        assert!(form.next_field(3));
        assert_eq!(form.field_index, 2);
        assert!(!form.next_field(3)); // Can't go beyond last field
        assert_eq!(form.field_index, 2);
    }

    #[test]
    fn test_previous_field() {
        let mut form = FormState::new(3);
        form.field_index = 2;
        assert!(form.previous_field());
        assert_eq!(form.field_index, 1);
        assert!(form.previous_field());
        assert_eq!(form.field_index, 0);
        assert!(!form.previous_field()); // Can't go before first field
        assert_eq!(form.field_index, 0);
    }

    #[test]
    fn test_push_pop_char() {
        let mut form = FormState::new(1);
        form.push_char('a');
        form.push_char('b');
        assert_eq!(form.current_value(), "ab");
        form.pop_char();
        assert_eq!(form.current_value(), "a");
        form.pop_char();
        assert_eq!(form.current_value(), "");
    }

    #[test]
    fn test_select_navigation() {
        let mut form = FormState::new(1);
        let matches = vec![
            ("Option 1".to_string(), "id1".to_string()),
            ("Option 2".to_string(), "id2".to_string()),
        ];
        form.set_select_matches(matches);

        form.select_next();
        assert_eq!(form.select_index, Some(0));
        form.select_next();
        assert_eq!(form.select_index, Some(1));
        form.select_next(); // Wrap around
        assert_eq!(form.select_index, Some(0));
    }

    #[test]
    fn test_reset_select_on_field_change() {
        let mut form = FormState::new(2);
        form.select_index = Some(0);
        form.next_field(2);
        assert!(form.select_index.is_none());
    }

    #[test]
    fn test_clear_form() {
        let mut form = FormState::new(3);
        form.push_char('a');
        form.next_field(3);
        form.push_char('b');
        form.clear();
        assert_eq!(form.field_index, 0);
        assert!(form.fields.iter().all(|f| f.is_empty()));
    }

    #[test]
    fn test_set_get_field_value() {
        let mut form = FormState::new(3);
        form.set_field_value(1, "test".to_string());
        assert_eq!(form.get_field_value(1), Some("test"));
    }
}
