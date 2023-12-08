# Mini Query

- Concerned about sqlx's [poor performance](https://github.com/diesel-rs/metrics/)?
- Not ready to leap into the depths of [SeaQuery](https://github.com/SeaQL/sea-query) just yet?
  - Or maybe it feels just a touch overkill for someone who is comfortable writing SQL?
- Do you want a simpler way to put your structs into your database and pull them back out again?

### Here's what it looks like

```rust
use mini_query::MiniQuery;
use tokio_postgres::{Row, GenericClient};
use anyhow::Result;

#[derive(MiniQuery, Default)]
#[mini_query(table_name = "users")]
struct User {
  #[mini_query(primary_key)]
  pub id: i32,
  #[mini_query(find_by)]
  pub email: String,
  #[mini_query(skip)]
  pub raw_password: Option<String>,
  #[mini_query(rename = "password")]
  enc_password: String,
  #[mini_query(cast = i16, get_by)] // this column is represented by a smallint in postgres
  role: UserRole
}
impl User {
  fn encrypt_password(&mut self) {
    let Some(raw_password) = &self.raw_password else {
      return;
    }
    self.enc_password = format!("{raw_password} - tada, I am encrypted");
  }
}

#[derive(Default)]
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

#[tokio::main]
async fn main() -> Result<()> {
  let (client, connection) =
    tokio_postgres::connect("host=localhost user=postgres", NoTls).await?;
  tokio::spawn(async move { connection.await });

  let mut user = User {
    email: "foo@dog.com".to_owned(),
    raw_password: "I am bad password".to_owned(),
    role: UserRole::Admin,
    ..Default::default()
  };
  user.encrypt_password();

  // fn is prefixed with "mini_" to avoid naming collisions, in case you wish to write your own.
  user.mini_insert(&client).await?;

  // look up user by email
  let same_user = User::find_by_email(&client, "foo@dog.com")?;
  assert_eq!(user.email, same_user.email);

  // get all the admins
  let admins = User::get_by_role(&client, &UserRole::Admin);
  assert_eq!(vec![same_user], admins);

  // get user by id and update
  let mut user = User::get(&client, &same_user.id);
  user.email = "bar@dog.com".to_owned();
  user.mini_update(&client).await?;

  // assert it saved
  assert_eq!(&User::get(&client, &user.id).email, "bar@dog.com");

  Ok(())
}

```

Only supports [tokio-postgres](https://docs.rs/tokio-postgres/latest/tokio_postgres/) right now. I might support more in the future, but we'll see.
