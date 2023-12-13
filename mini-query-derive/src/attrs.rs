use convert_case::{Case, Casing};
use syn::{meta::ParseNestedMeta, AttrStyle, Attribute, LitStr, Result, Type};

pub trait ParseAttributes: Sized {
  fn default(attribute: &'static str) -> Self;
  fn parse_attribute(&mut self, m: ParseNestedMeta) -> Result<()>;

  fn parse_attributes(attribute_name: &'static str, attributes: &[Attribute]) -> Result<Self> {
    let mut res = Self::default(attribute_name);

    for attribute in attributes {
      if !matches!(attribute.style, AttrStyle::Outer) {
        continue;
      }

      if attribute.path().is_ident(attribute_name) {
        attribute.parse_nested_meta(|meta| res.parse_attribute(meta))?;
      }
    }

    Ok(res)
  }
}

#[derive(Default)]
pub struct ContainerAttributes {
  attribute: &'static str,
  rename_all: Option<Case>,
  pub table_name: Option<String>,
}

impl ContainerAttributes {
  pub fn apply_to_field(&self, field: &str) -> String {
    let Some(case) = self.rename_all else {
      return field.to_owned();
    };
    field.to_case(case)
  }

  pub fn attribute(&self) -> &'static str {
    self.attribute
  }
}

impl ParseAttributes for ContainerAttributes {
  fn default(attribute: &'static str) -> Self {
    Self {
      attribute,
      ..Default::default()
    }
  }

  fn parse_attribute(&mut self, m: ParseNestedMeta) -> Result<()> {
    if m.path.is_ident("rename_all") {
      self.rename_all = Some(case_from_str(&m.value()?.parse::<LitStr>()?.value()));
    } else if m.path.is_ident("table_name") {
      self.table_name = Some(m.value()?.parse::<LitStr>()?.value());
    } else {
      return Err(m.error("unknown attribute"));
    }

    Ok(())
  }
}

#[derive(Default)]
pub struct FieldAttribute {
  pub rename: Option<String>,
  pub cast: Option<Type>,
  pub primary_key: bool,
  pub get_by: bool,
  pub find_by: bool,
  skip: bool,
}

impl FieldAttribute {
  pub fn apply_to_field(&self, field: &str) -> Option<String> {
    if self.skip {
      return None;
    }

    if let Some(rename) = &self.rename {
      return Some(rename.to_owned());
    }

    Some(field.to_owned())
  }
}

impl ParseAttributes for FieldAttribute {
  fn default(_attribute: &'static str) -> Self {
    Self { ..Default::default() }
  }

  fn parse_attribute(&mut self, m: ParseNestedMeta) -> Result<()> {
    if m.path.is_ident("rename") {
      self.rename = Some(m.value()?.parse::<LitStr>()?.value());
    } else if m.path.is_ident("skip") {
      self.skip = true;
    } else if m.path.is_ident("cast") {
      self.cast = Some(m.value()?.parse::<Type>()?);
    } else if m.path.is_ident("primary_key") {
      self.primary_key = true;
    } else if m.path.is_ident("get_by") {
      self.get_by = true;
    } else if m.path.is_ident("find_by") {
      self.find_by = true;
    } else {
      return Err(m.error("unknown attribute"));
    }

    Ok(())
  }
}

fn case_from_str(s: &str) -> Case {
  match s {
    "lowercase" => Case::Lower,
    "UPPERCASE" => Case::Upper,
    "PascalCase" => Case::Pascal,
    "camelCase" => Case::Camel,
    "snake_case" => Case::Snake,
    "SCREAMING_SNAKE_CASE" => Case::UpperSnake,
    "kebab-case" => Case::Kebab,
    "SCREAMING-KEBAB-CASE" => Case::UpperKebab,
    _ => panic!("unable to parse rename_all rule: {s}"),
  }
}
