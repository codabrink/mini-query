# Mini Query

[![Latest Version](https://img.shields.io/crates/v/mini-query.svg)](https://crates.io/crates/mini-query)

A mini ORM to quickly find / insert / retrieve records with tokio-postgres.

## Generates the following functions on the struct:

If #[mini_query(primary_key)] is set:

```rust
get(id: &T) -> Result<Option<T>>
```

For all fields marked with #[mini_query(find_by)]:

```rust
find_by_{x}(client: &impl GenericClient, val: &T) -> Result<Option<T>>
```

For all fields marked with #[mini_query(get_by)]:

```rust
get_by_{x}(client: &impl GenericClient, val: &T) -> Result<Vec<T>>
```

And of course, support for inserting and updating.

```rust
quick_insert(&self, client: &impl GenericClient) -> Result<Self>
```

```rust
quick_insert_no_return(&self, client: &impl GenericClient) -> Result<()>
```

```rust
quick_update(&self, client: &impl GenericClient) -> Result<Self>
```

This macro also implements the From\<Row> trait for your struct. Making this possible:

```rust
let user: User = client.query_one("SELECT * FROM users WHERE id = $1", &[&1]).await?.into();
```

## Here's an example for a "users" table

```rust
use chrono::prelude::*;
use mini_query::MiniQuery;
use tokio_postgres::GenericClient;

#[derive(MiniQuery, Default)]
#[mini_query(table_name = "users")]
struct User {
  #[mini_query(primary_key)]
  pub id: i32,
  #[mini_query(find_by)]
  pub email: String,
  #[mini_query(skip)]
  pub raw_password: Option<String>,
  #[mini_query(rename = "password")] // renames field to "password" when saving
  pub enc_password: String,
  #[mini_query(cast = i16, get_by)] // field is represented by a smallint in postgres
  pub role: UserRole,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}
impl User {
  fn encrypt_password(&mut self) {
    let Some(raw_password) = &self.raw_password else {
      return;
    }
    self.enc_password = format!("{raw_password} - tada, I am encrypted");
  }
}

#[derive(Default, Clone, Copy)]
#[repr(i16)]
enum UserRole {
  #[default]
  User = 0,
  Admin = 1
}
impl From<i16> for UserRole {
  fn from(val: i16) -> Self {
    match val {
      0 => UserRole::User,
      1 => UserRole::Admin,
      _ => unimplemented!(),
    }
  }
}

#[derive(MiniQuery, Default)]
#[mini_query(table_name = "posts")]
struct Post {
  #[mini_query(primary_key)]
  pub user_id: i32,
  pub content: String
}
impl Post {
  belongs_to!(User, user, user_id);
}

#[tokio::main]
async fn main() -> Result<()> {
  let (client, connection) =
        tokio_postgres::connect("postgresql://postgres@localhost/mydb-dev", NoTls).await?;
    tokio::spawn(async move { connection.await });

    let mut user = User {
        email: "foo@dog.com".to_owned(),
        raw_password: Some("I am bad password".to_owned()),
        role: UserRole::Admin,
        ..Default::default()
    };
    user.encrypt_password();

    // fn is prefixed with "quick_" to avoid naming collisions',
    // in case you wish to write your own validation wrapper.
    user.quick_insert(&client).await?;

    // find user by email
    let same_user = User::find_by_email(&client, "foo@dog.com").await?.unwrap();
    assert_eq!(user.email, same_user.email);

    // get all the admins
    let admin = User::get_by_role(&client, &UserRole::Admin)
        .await?
        .pop()
        .unwrap();
    assert_eq!(same_user.email, admin.email);

    // get user by id and update
    let mut user = User::get(&client, &same_user.id).await?.unwrap();
    user.email = "bar@dog.com".to_owned();
    user.quick_update(&client).await?;

    // assert it saved
    assert_eq!(
        &User::get(&client, &user.id).await?.unwrap().email,
        "bar@dog.com"
    );

  Ok(())
}

```

Only supports [tokio-postgres](https://docs.rs/tokio-postgres/latest/tokio_postgres/) right now.
