//! Organization form handling for TUI.

use crate::error::BalebosError;
use crate::org::BrandId;
use crate::tui::app::{App, InputMode, OrgAddType};

impl App {
    pub fn start_org_add(&mut self, add_type: OrgAddType) {
        let field_count = add_type.fields().len();
        self.form = crate::tui::state::FormState::new(field_count);
        self.input_mode = InputMode::OrgAdding(add_type);
        self.update_select_matches();
    }

    pub fn start_org_edit(&mut self, add_type: OrgAddType, index: usize) {
        let mut form = crate::tui::state::FormState::new(add_type.fields().len());
        match add_type {
            OrgAddType::Person => {
                if let Some(p) = self.org_model.persons.get(index) {
                    form.set_field_value(0, p.first_name.clone());
                    form.set_field_value(1, p.last_name.clone());
                    form.set_field_value(2, p.email.clone());
                    form.set_field_value(3, p.phone.clone().unwrap_or_default());
                }
            }
            OrgAddType::Company => {
                if let Some(c) = self.org_model.companies.get(index) {
                    form.set_field_value(0, c.name.clone());
                    form.set_field_value(1, c.country.alpha2().to_string());
                    form.set_field_value(
                        2,
                        c.brand
                            .map(|id: BrandId| id.to_string())
                            .unwrap_or_default(),
                    );
                }
            }
            OrgAddType::Brand => {
                if let Some(b) = self.org_model.brands.get(index) {
                    form.set_field_value(0, b.name.clone());
                }
            }
            OrgAddType::Project => {
                if let Some(p) = self.org_model.projects.get(index) {
                    form.set_field_value(0, p.name.clone());
                    form.set_field_value(1, p.company.to_string());
                    form.set_field_value(
                        2,
                        p.importance.map(|i: u8| i.to_string()).unwrap_or_default(),
                    );
                }
            }
        }
        self.form = form;
        self.input_mode = InputMode::OrgEditing(add_type, index);
        self.update_select_matches();
    }

    pub fn form_next_field(&mut self) {
        if let InputMode::OrgAdding(add_type) | InputMode::OrgEditing(add_type, _) =
            &self.input_mode
        {
            let max = add_type.fields().len();
            if self.form.next_field(max) {
                self.update_select_matches();
            }
        }
    }

    pub fn form_prev_field(&mut self) {
        if self.form.previous_field() {
            self.update_select_matches();
        }
    }

    pub fn org_select_next(&mut self) {
        self.form.select_next();
    }

    pub fn org_select_prev(&mut self) {
        self.form.select_previous();
    }

    pub fn update_select_matches(&mut self) {
        let mode = match &self.input_mode {
            InputMode::OrgAdding(t) => Some(*t),
            InputMode::OrgEditing(t, _) => Some(*t),
            InputMode::Assigning => {
                let current_input = self.input.to_lowercase();
                let matches = self
                    .org_model
                    .projects
                    .iter()
                    .map(|p| {
                        let brand_name = self
                            .org_model
                            .companies_by_id
                            .get(&p.company)
                            .and_then(|c| c.brand)
                            .and_then(|b_id| self.org_model.brands_by_id.get(&b_id))
                            .map(|b| b.name.clone())
                            .unwrap_or_else(|| "Unknown".to_string());
                        (format!("{} ({})", p.name, brand_name), p.id.to_string())
                    })
                    .filter(|(display, _id): &(String, String)| {
                        display.to_lowercase().contains(&current_input)
                    })
                    .collect();

                self.form.set_select_matches(matches);
                return;
            }
            _ => None,
        };

        if let Some(add_type) = mode {
            if add_type.is_id_field(self.form.field_index) {
                let current_input = self
                    .form
                    .get_field_value(self.form.field_index)
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();

                let candidates = add_type.get_candidates(self.form.field_index, self);
                let matches = candidates
                    .into_iter()
                    .filter(|(name, _id)| name.to_lowercase().contains(&current_input))
                    .collect();

                self.form.set_select_matches(matches);
            } else {
                self.form.reset_select();
            }
        }
    }

    #[allow(dead_code)]
    pub fn form_current_value(&self) -> &str {
        self.form.current_value()
    }

    pub fn form_push_char(&mut self, c: char) {
        self.form.push_char(c);
        self.update_select_matches();
    }

    pub fn form_pop_char(&mut self) {
        self.form.pop_char();
        self.update_select_matches();
    }

    pub fn cancel_form(&mut self) {
        self.input_mode = InputMode::Normal;
        self.form.reset();
    }

    /// Submit the org form and create the entity.
    /// Returns Ok(true) if successful, Ok(false) if validation failed.
    pub fn submit_org_form(&mut self) -> Result<bool, BalebosError> {
        // If we have a selection, pick it instead of submitting the form
        if let Some((_, id)) = self.form.selected_match() {
            let id = id.clone();
            self.form.set_field_value(self.form.field_index, id);
            self.form.reset_select();
            // Auto-advance to next field if possible
            self.form_next_field();
            return Ok(true);
        }

        let mode = self.input_mode.clone();
        let (add_type, edit_index) = match &mode {
            InputMode::OrgAdding(t) => (*t, None),
            InputMode::OrgEditing(t, idx) => (*t, Some(*idx)),
            _ => return Ok(false),
        };

        let success = match add_type {
            OrgAddType::Person => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.persons.get(idx) {
                        self.submit_person_edit(existing.id)?
                    } else {
                        false
                    }
                } else {
                    self.submit_person_form()?
                }
            }
            OrgAddType::Company => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.companies.get(idx) {
                        self.submit_company_edit(existing.id)?
                    } else {
                        false
                    }
                } else {
                    self.submit_company_form()?
                }
            }
            OrgAddType::Brand => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.brands.get(idx) {
                        self.submit_brand_edit(existing.id)?
                    } else {
                        false
                    }
                } else {
                    self.submit_brand_form()?
                }
            }
            OrgAddType::Project => {
                if let Some(idx) = edit_index {
                    if let Some(existing) = self.org_model.projects.get(idx) {
                        self.submit_project_edit(existing.id)?
                    } else {
                        false
                    }
                } else {
                    self.submit_project_form()?
                }
            }
        };

        if success {
            self.input_mode = InputMode::Normal;
            self.refresh()?;
        }
        Ok(success)
    }

    fn submit_person_form(&mut self) -> Result<bool, BalebosError> {
        let first_name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let last_name = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let email = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");
        let phone_str = self.form.get_field_value(3).map(|s| s.trim()).unwrap_or("");
        let phone = if phone_str.is_empty() {
            None
        } else {
            Some(phone_str.to_string())
        };

        if first_name.is_empty() || last_name.is_empty() || email.is_empty() {
            return Ok(false);
        }

        crate::app::org::add_person(&self.storage, first_name, last_name, email, phone)?;
        Ok(true)
    }

    fn submit_person_edit(&mut self, id: crate::org::PersonId) -> Result<bool, BalebosError> {
        let first_name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let last_name = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let email = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");
        let phone_str = self.form.get_field_value(3).map(|s| s.trim()).unwrap_or("");
        let phone = if phone_str.is_empty() {
            None
        } else {
            Some(phone_str.to_string())
        };

        if first_name.is_empty() || last_name.is_empty() || email.is_empty() {
            return Ok(false);
        }

        crate::app::org::update_person(&self.storage, id, first_name, last_name, email, phone)?;
        Ok(true)
    }

    fn submit_company_form(&mut self) -> Result<bool, BalebosError> {
        use crate::org::{BrandId, CountryCode};

        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let country_str = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let brand_str = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");

        if name.is_empty() || country_str.is_empty() {
            return Ok(false);
        }

        let country = match CountryCode::for_alpha2(country_str) {
            Ok(c) => c,
            Err(_) => return Ok(false), // Invalid country code
        };

        // Brand is optional - empty means external contractor
        let brand: Option<BrandId> = if brand_str.is_empty() {
            None
        } else {
            match brand_str.parse() {
                Ok(id) => Some(id),
                Err(_) => return Ok(false), // Invalid brand ID
            }
        };

        crate::app::org::add_company(&self.storage, name, country, brand)?;
        Ok(true)
    }

    fn submit_company_edit(&mut self, id: crate::org::CompanyId) -> Result<bool, BalebosError> {
        use crate::org::{BrandId, CountryCode};

        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let country_str = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let brand_str = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");

        if name.is_empty() || country_str.is_empty() {
            return Ok(false);
        }

        let country = match CountryCode::for_alpha2(country_str) {
            Ok(c) => c,
            Err(_) => return Ok(false), // Invalid country code
        };

        let brand: Option<BrandId> = if brand_str.is_empty() {
            None
        } else {
            match brand_str.parse() {
                Ok(id) => Some(id),
                Err(_) => return Ok(false), // Invalid brand ID
            }
        };

        crate::app::org::update_company(&self.storage, id, name, country, brand)?;
        Ok(true)
    }

    fn submit_brand_form(&mut self) -> Result<bool, BalebosError> {
        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");

        // Validation
        if name.is_empty() {
            return Ok(false);
        }

        crate::app::org::add_brand(&self.storage, name)?;
        Ok(true)
    }

    fn submit_brand_edit(&mut self, id: crate::org::BrandId) -> Result<bool, BalebosError> {
        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");

        if name.is_empty() {
            return Ok(false);
        }

        crate::app::org::update_brand(&self.storage, id, name)?;
        Ok(true)
    }

    fn parse_importance(&self) -> Option<u8> {
        let imp_str = self.form.get_field_value(2).map(|s| s.trim()).unwrap_or("");
        if imp_str.is_empty() {
            return None;
        }
        match imp_str.parse::<u8>() {
            Ok(v) if (1..=5).contains(&v) => Some(v),
            _ => None,
        }
    }

    fn submit_project_form(&mut self) -> Result<bool, BalebosError> {
        use crate::org::CompanyId;

        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let company_id_str = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let importance = self.parse_importance();

        if name.is_empty() || company_id_str.is_empty() {
            return Ok(false);
        }

        let company_id = match company_id_str.parse::<CompanyId>() {
            Ok(id) => id,
            Err(_) => return Ok(false),
        };

        crate::app::org::add_project_with_importance(&self.storage, name, company_id, importance)?;
        Ok(true)
    }

    fn submit_project_edit(&mut self, id: crate::org::ProjectId) -> Result<bool, BalebosError> {
        use crate::org::CompanyId;

        let name = self.form.get_field_value(0).map(|s| s.trim()).unwrap_or("");
        let company_id_str = self.form.get_field_value(1).map(|s| s.trim()).unwrap_or("");
        let importance = self.parse_importance();

        if name.is_empty() || company_id_str.is_empty() {
            return Ok(false);
        }

        let company_id = match company_id_str.parse::<CompanyId>() {
            Ok(id) => id,
            Err(_) => return Ok(false),
        };

        crate::app::org::set_project_importance(&self.storage, id, name, company_id, importance)?;
        Ok(true)
    }
}
