extern crate proc_macro;
mod attrs;

use attrs::{ContainerAttributes, FieldAttribute, ParseAttributes};
use core::panic;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Error, Fields, FieldsNamed};

#[proc_macro_derive(MiniQuery, attributes(mini_query))]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
  let input = parse_macro_input!(input as DeriveInput);

  let struct_name = &input.ident;

  let (_impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

  let container_attributes = ContainerAttributes::parse_attributes("mini_query", &input.attrs).unwrap();

  let Some(table_name) = &container_attributes.table_name else {
    panic!("Expected table_name attr.");
  };

  let Data::Struct(DataStruct {
    fields: Fields::Named(FieldsNamed { named: fields, .. }),
    ..
  }) = input.data
  else {
    panic!("Derive(MiniQuery) only applicable to named structs");
  };

  let mut token_stream = TokenStream::new();

  let mut primary_key = None;
  let mut field_tokens = Vec::new();
  let mut field_names = Vec::new();
  let mut from_impl = Vec::new();

  for field in fields {
    let field_attributes = FieldAttribute::parse_attributes(container_attributes.attribute(), &field.attrs).unwrap();

    let ty = &field.ty;
    let Some(field) = field.ident else {
      return Err(Error::new_spanned(field, "field must be a named field")).unwrap();
    };

    let name = container_attributes.apply_to_field(&field.to_string());

    // is this field is set as the primary key?
    // denoted with #[mini_query(primary_key)]
    if field_attributes.primary_key {
      primary_key = Some((field.clone(), ty.clone()));
      from_impl.push(TokenStream::from(quote! { #field: row.get(stringify!(#field)) }));
      continue;
    }

    // this block will be skipped on this field if #[mini_query(skip)] is set
    if let Some(name) = field_attributes.apply_to_field(&name) {
      field_names.push(name.clone());

      // is the field in question being casted when sent to / from database?
      // denoted with #[mini_query(cast = i16)]
      if let Some(cast) = field_attributes.cast {
        from_impl.push(TokenStream::from(quote! { #field: row.get::<'a, &str, #cast>(#name).into() }));
        field_tokens.push(TokenStream::from(quote! { #field as #cast }));
      } else {
        from_impl.push(TokenStream::from(quote! { #field: row.get(#name) }));
        field_tokens.push(TokenStream::from(quote! { #field }));
      }
    }

    // build out the get_by_x functions
    // denoted with #[mini_query(get_by)]
    if field_attributes.get_by {
      let field = field.clone();
      let query = format!("SELECT * FROM {table_name} WHERE {name} = $1");
      let get_by_fn_name = format_ident!("get_by_{}", field);

      token_stream.extend(TokenStream::from(quote! {
        impl #struct_name #type_generics #where_clause {
          pub async fn #get_by_fn_name(client: &impl GenericClient, field: &#ty) -> anyhow::Result<Vec<Self>> {
            let recs = client.query(#query, &[&field]).await?.iter().map(Self::from).collect();
            Ok(recs)

          }
        }
      }));
    }

    // build out the find_by_x functions
    // denoted with #[mini_query(find_by)]
    if field_attributes.find_by {
      let field = field.clone();
      let query = format!("SELECT * FROM {table_name} WHERE {name} = $1");
      let find_by_fn_name = format_ident!("find_by_{}", field);

      token_stream.extend(TokenStream::from(quote! {
        impl #struct_name #type_generics #where_clause {
          pub async fn #find_by_fn_name(client: &impl GenericClient, field: &#ty) -> anyhow::Result<Option<Self>> {
            let rec = client.query_opt(#query, &[&field]).await?.map(Self::from);
            Ok(rec)
          }
        }
      }));
    }
  }

  let len = field_names.len();

  let ts = TokenStream::from(quote! {
      impl From<tokio_postgres::Row> for #struct_name #type_generics #where_clause {
        fn from(row: tokio_postgres::Row) -> Self {
          Self::from(&row)
        }
      }
      impl<'a> From<&'a tokio_postgres::Row> for #struct_name #type_generics #where_clause {
        fn from(row: &'a tokio_postgres::Row) -> Self {
          Self {
            #(#from_impl),*,
            ..Default::default()
          }
        }
      }
  });
  token_stream.extend(ts);

  let ts = {
    let field_tokens = field_tokens.clone();

    let dollar_signs: String = (1..=len).map(|i| format!("${i}")).collect::<Vec<String>>().join(",");
    let insert_query = format!("INSERT INTO {table_name} ({}) VALUES ({dollar_signs})", field_names.join(","));
    let insert_query_returning = format!("{} RETURNING *", &insert_query);
    let all_query = format!("SELECT * FROM {table_name}");

    TokenStream::from(quote! {
      impl #struct_name #type_generics #where_clause {
        #[doc=concat!("Generated array of field names for `", stringify!(#struct_name #type_generics), "`.")]
        const FIELD_NAMES: [&'static str; #len] = [#(#field_names),*];
        pub const TABLE_NAME: &'static str = #table_name;

        pub async fn all(client: &impl GenericClient) -> anyhow::Result<Vec<Self>> {
          let recs = client.query(#all_query, &[]).await?.iter().map(Self::from).collect();
          Ok(recs)
        }

        pub async fn quick_insert(&self, client: &impl GenericClient) -> anyhow::Result<Self> {
          let rec = client.query_one(
            #insert_query_returning,
            &[#(&(self.#field_tokens)),*]
          ).await?;

          Ok(Self::from(rec))
        }

        pub async fn quick_insert_no_return(&self, client: &impl GenericClient) -> anyhow::Result<()> {
          client
            .query(
              #insert_query,
              &[#(&(self.#field_tokens)),*]
            ).await?;

          Ok(())
        }
      }
    })
  };
  token_stream.extend(ts);

  if let Some((ident, ty)) = primary_key {
    let query = format!("SELECT * FROM {} WHERE {} = $1", table_name, ident);
    let update_query = field_names
      .iter()
      .enumerate()
      .map(|(i, name)| format!("{name}=${}", i + 2))
      .collect::<Vec<String>>()
      .join(",");
    let update_query = format!("UPDATE {table_name} SET {update_query} WHERE id=$1 RETURNING *");

    let ts = TokenStream::from(quote! {
      impl #struct_name #type_generics #where_clause {
        pub async fn get(client: &impl GenericClient, id: &#ty) -> anyhow::Result<Option<Self>> {
          let rec = client.query_opt(#query, &[&id]).await?.map(Self::from);

          Ok(rec)
        }

        pub async fn quick_update(&self, client: &impl GenericClient) -> anyhow::Result<Self> {
          let rec = client.query_one(#update_query, &[&self.#ident, #(&(self.#field_tokens)),*]).await?;

          Ok(Self::from(rec))
        }
      }
    });
    token_stream.extend(ts);
  }

  token_stream.into()
}
