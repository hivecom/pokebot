use std::time::{Duration, SystemTime};

use diesel::{
    backend::Backend,
    deserialize::{FromSql, FromSqlRow},
    expression::AsExpression,
    serialize::{Output, ToSql},
    sql_types::{BigInt, Nullable},
};
use serde::{Deserialize, Deserializer, Serializer};

pub fn unix_timestamp() -> i64 {
    SystemTime::UNIX_EPOCH.elapsed().unwrap().as_secs() as i64
}

pub fn serialize_opt_duration<S>(option: &Option<Duration>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match option {
        Some(v) => s.serialize_u64(v.as_millis().try_into().unwrap()),
        None => s.serialize_none(),
    }
}

pub fn deserialize_opt_duration<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let duration = Option::<u64>::deserialize(d)?.map(Duration::from_millis);

    Ok(duration)
}

pub fn schema_opt_duration() -> i64 {
    // FIXME: huh?
    5
}

pub fn serialize_duration<S>(dur: &Duration, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_u64(dur.as_millis().try_into().unwrap())
}

pub fn schema_duration() -> i64 {
    // FIXME: huh?
    5
}

#[derive(Debug, FromSqlRow, AsExpression)]
#[diesel(sql_type = BigInt)]
pub struct DbDuration(i64);

impl From<Duration> for DbDuration {
    fn from(val: Duration) -> Self {
        DbDuration(val.as_millis() as i64)
    }
}

impl From<DbDuration> for Duration {
    fn from(val: DbDuration) -> Self {
        Duration::from_millis(val.0 as u64)
    }
}

impl<DB> ToSql<BigInt, DB> for DbDuration
where
    DB: Backend,
    i64: ToSql<BigInt, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB> FromSql<BigInt, DB> for DbDuration
where
    DB: Backend,
    i64: FromSql<BigInt, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let v = i64::from_sql(bytes)?;

        Ok(DbDuration(v))
    }
}

#[derive(Debug, FromSqlRow, AsExpression)]
#[diesel(sql_type = Nullable<BigInt>)]
pub struct DbOptDuration(Option<i64>);

impl From<Option<Duration>> for DbOptDuration {
    fn from(val: Option<Duration>) -> Self {
        DbOptDuration(val.map(|v| v.as_millis() as i64))
    }
}

impl From<DbOptDuration> for Option<Duration> {
    fn from(val: DbOptDuration) -> Self {
        val.0.map(|v| Duration::from_millis(v as u64))
    }
}

impl<DB> ToSql<Nullable<BigInt>, DB> for DbOptDuration
where
    DB: Backend,
    Option<i64>: ToSql<Nullable<BigInt>, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB> FromSql<Nullable<BigInt>, DB> for DbOptDuration
where
    DB: Backend,
    Option<i64>: FromSql<Nullable<BigInt>, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let v = Option::<i64>::from_sql(bytes)?;

        Ok(DbOptDuration(v))
    }
}
