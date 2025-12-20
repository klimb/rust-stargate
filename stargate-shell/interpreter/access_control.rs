use super::super::scripting::*;
use super::Interpreter;

impl Interpreter {
    pub(super) fn can_access_field(
        &self,
        target_class: &str,
        field_name: &str,
        access: &AccessModifier,
    ) -> Result<(), String> {
        match access {
            AccessModifier::Public => Ok(()),
            
            AccessModifier::Private => {
                if let Some(ref current) = self.current_class_context {
                    if current == target_class {
                        return Ok(());
                    }
                }
                Err(format!(
                    "Cannot access private field '{}' of class '{}'",
                    field_name, target_class
                ))
            }
            
            AccessModifier::Protected => {
                if let Some(ref current) = self.current_class_context {
                    if current == target_class || self.is_subclass_of(current, target_class) {
                        return Ok(());
                    }
                }
                Err(format!(
                    "Cannot access protected field '{}' of class '{}' from outside class hierarchy",
                    field_name, target_class
                ))
            }
        }
    }
    
    pub(super) fn can_call_method(
        &self,
        target_class: &str,
        method_name: &str,
        access: &AccessModifier,
    ) -> Result<(), String> {
        match access {
            AccessModifier::Public => Ok(()),
            
            AccessModifier::Private => {
                if let Some(ref current) = self.current_class_context {
                    if current == target_class {
                        return Ok(());
                    }
                }
                Err(format!(
                    "Cannot call private method '{}' of class '{}'",
                    method_name, target_class
                ))
            }
            
            AccessModifier::Protected => {
                if let Some(ref current) = self.current_class_context {
                    if current == target_class || self.is_subclass_of(current, target_class) {
                        return Ok(());
                    }
                }
                Err(format!(
                    "Cannot call protected method '{}' of class '{}' from outside class hierarchy",
                    method_name, target_class
                ))
            }
        }
    }
    
    pub(super) fn can_call_function(
        &self,
        function_name: &str,
        access: &AccessModifier,
    ) -> Result<(), String> {
        match access {
            AccessModifier::Public => Ok(()),
            AccessModifier::Private | AccessModifier::Protected => {
                Err(format!(
                    "Cannot call non-public function '{}' from external context",
                    function_name
                ))
            }
        }
    }
    
    fn is_subclass_of(&self, current_class: &str, potential_parent: &str) -> bool {
        let mut current = Some(current_class.to_string());
        
        while let Some(ref cls) = current {
            if cls == potential_parent {
                return true;
            }
            
            if let Some((parent, _, _)) = self.classes.get(cls) {
                current = parent.clone();
            } else {
                break;
            }
        }
        
        false
    }
    
    pub(super) fn find_field_with_access(
        &self,
        class_name: &str,
        field_name: &str,
    ) -> Result<(AccessModifier, Expression), String> {
        let mut current_class = Some(class_name.to_string());
        
        while let Some(ref cls) = current_class {
            if let Some((parent, fields, _)) = self.classes.get(cls) {
                for (access, name, expr) in fields {
                    if name == field_name {
                        return Ok((access.clone(), expr.clone()));
                    }
                }
                current_class = parent.clone();
            } else {
                break;
            }
        }
        
        Err(format!("Field '{}' not found in class '{}'", field_name, class_name))
    }
    
    pub(super) fn find_method_with_access(
        &self,
        class_name: &str,
        method_name: &str,
    ) -> Result<(AccessModifier, Vec<String>, Vec<Statement>), String> {
        let mut current_class = Some(class_name.to_string());
        
        while let Some(ref cls) = current_class {
            if let Some((parent, _, methods)) = self.classes.get(cls) {
                for (access, name, params, body) in methods {
                    if name == method_name {
                        return Ok((access.clone(), params.clone(), body.clone()));
                    }
                }
                current_class = parent.clone();
            } else {
                break;
            }
        }
        
        Err(format!("Method '{}' not found in class '{}'", method_name, class_name))
    }
}
