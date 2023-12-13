pub use mini_query_derive::MiniQuery;

#[macro_export]
macro_rules! has_many {
  ($child:ident, $fn_name:ident, $pk:ident, $fk:ident) => {
    pub async fn $fn_name(&self, client: &impl GenericClient) -> anyhow::Result<Vec<$child>> {
      let recs = client
        .query(
          &format!("SELECT * FROM {} WHERE {} = $1", $child::__TABLE_NAME__, stringify!($fk)),
          &[&self.$pk],
        )
        .await?
        .iter()
        .map($child::from)
        .collect();
      Ok(recs)
    }
  };
}

#[macro_export]
macro_rules! belongs_to {
  ($parent:ident, $fn_name:ident, $pk:ident, $fk:ident) => {
    pub async fn $fn_name(&self, client: &impl GenericClient) -> anyhow::Result<Option<$parent>> {
      let rec = client
        .query_opt(
          &format!("SELECT * FROM {} WHERE {} = $1", $parent::__TABLE_NAME__, stringify!($pk)),
          &[&self.$fk],
        )
        .await?
        .map($parent::from);
      Ok(rec)
    }
  };
}
