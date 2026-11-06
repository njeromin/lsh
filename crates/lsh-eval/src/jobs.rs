use std::collections::BTreeMap;

use lsh_sys::signals::{ProcessStatus, wait_pid_status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    Running,
    Stopped(i32),
    Done(i32),
}

#[derive(Clone)]
pub struct Job {
    pub id: usize,
    pub pgid: i32,
    pub pids: Vec<i32>,
    pub command: String,
    pub state: JobState,
    pub notified: bool,
    #[cfg(unix)]
    pub saved_termios: Option<libc::termios>,
    #[cfg(not(unix))]
    pub saved_termios: Option<()>,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("id", &self.id)
            .field("pgid", &self.pgid)
            .field("pids", &self.pids)
            .field("command", &self.command)
            .field("state", &self.state)
            .field("notified", &self.notified)
            .field("saved_termios", &self.saved_termios.is_some())
            .finish()
    }
}

#[derive(Default)]
pub struct JobManager {
    pub jobs: BTreeMap<usize, Job>,
    pub current_job: Option<usize>,
    pub previous_job: Option<usize>,
}

impl JobManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&self) -> usize {
        let mut id = 1;
        while self.jobs.contains_key(&id) {
            id += 1;
        }
        id
    }

    pub fn add_job(
        &mut self,
        pgid: i32,
        pids: Vec<i32>,
        command: String,
        state: JobState,
        #[cfg(unix)] saved_termios: Option<libc::termios>,
        #[cfg(not(unix))] saved_termios: Option<()>,
    ) -> usize {
        let id = self.next_id();
        let job = Job {
            id,
            pgid,
            pids,
            command,
            state,
            notified: false,
            saved_termios,
        };
        self.jobs.insert(id, job);
        self.update_current_after_insert(id);
        id
    }

    fn update_current_after_insert(&mut self, new_id: usize) {
        if self.current_job != Some(new_id) {
            self.previous_job = self.current_job;
            self.current_job = Some(new_id);
        }
    }

    pub fn remove_job(&mut self, id: usize) -> Option<Job> {
        let removed = self.jobs.remove(&id);
        if removed.is_some() {
            if self.current_job == Some(id) {
                self.current_job = self.previous_job;
                self.previous_job = self.find_fallback_previous();
            } else if self.previous_job == Some(id) {
                self.previous_job = self.find_fallback_previous();
            }
        }
        removed
    }

    fn find_fallback_previous(&self) -> Option<usize> {
        self.jobs
            .keys()
            .rev()
            .find(|&&k| Some(k) != self.current_job)
            .copied()
    }

    pub fn get_job(&self, id: usize) -> Option<&Job> {
        self.jobs.get(&id)
    }

    pub fn get_job_mut(&mut self, id: usize) -> Option<&mut Job> {
        self.jobs.get_mut(&id)
    }

    pub fn find_by_pgid(&self, pgid: i32) -> Option<&Job> {
        self.jobs.values().find(|j| j.pgid == pgid)
    }

    pub fn find_by_pgid_mut(&mut self, pgid: i32) -> Option<&mut Job> {
        self.jobs.values_mut().find(|j| j.pgid == pgid)
    }

    pub fn find_by_pid_mut(&mut self, pid: i32) -> Option<&mut Job> {
        self.jobs.values_mut().find(|j| j.pids.contains(&pid))
    }

    pub fn find_by_spec(&self, spec: &str) -> Result<usize, String> {
        let trimmed = spec.trim();
        if trimmed.is_empty()
            || trimmed == "%"
            || trimmed == "%%"
            || trimmed == "%+"
            || trimmed == "+"
        {
            return self
                .current_job
                .ok_or_else(|| "current: no such job".to_string());
        }

        if trimmed == "%-" || trimmed == "-" {
            return self
                .previous_job
                .ok_or_else(|| "previous: no such job".to_string());
        }

        let arg = trimmed.strip_prefix('%').unwrap_or(trimmed);

        if let Ok(id) = arg.parse::<usize>() {
            if self.jobs.contains_key(&id) {
                return Ok(id);
            }
            return Err(format!("{spec}: no such job"));
        }

        if let Some(prefix) = arg.strip_prefix('?') {
            let matches: Vec<usize> = self
                .jobs
                .iter()
                .filter(|(_, j)| j.command.contains(prefix))
                .map(|(&id, _)| id)
                .collect();
            if matches.len() == 1 {
                return Ok(matches[0]);
            } else if matches.is_empty() {
                return Err(format!("{spec}: no such job"));
            } else {
                return Err(format!("{spec}: ambiguous job spec"));
            }
        }

        let matches: Vec<usize> = self
            .jobs
            .iter()
            .filter(|(_, j)| j.command.starts_with(arg))
            .map(|(&id, _)| id)
            .collect();
        if matches.len() == 1 {
            Ok(matches[0])
        } else if matches.is_empty() {
            Err(format!("{spec}: no such job"))
        } else {
            Err(format!("{spec}: ambiguous job spec"))
        }
    }

    pub fn has_stopped_jobs(&mut self) -> bool {
        self.reap_jobs();
        self.jobs
            .values()
            .any(|j| matches!(j.state, JobState::Stopped(_)))
    }

    pub fn reap_jobs(&mut self) -> Vec<(usize, JobState, String)> {
        let mut notifications = Vec::new();

        #[cfg(unix)]
        loop {
            let res = wait_pid_status(-1, libc::WNOHANG | libc::WUNTRACED | libc::WCONTINUED);
            match res {
                Ok((pid, status)) if pid > 0 => {
                    let mut job_id_and_cmd = None;
                    let mut new_state = None;

                    if let Some(job) = self.find_by_pid_mut(pid) {
                        match status {
                            ProcessStatus::Exited(code) => {
                                job.state = JobState::Done(code);
                                new_state = Some(JobState::Done(code));
                                job_id_and_cmd = Some((job.id, job.command.clone()));
                            }
                            ProcessStatus::Signaled(sig) => {
                                let code = 128 + sig;
                                job.state = JobState::Done(code);
                                new_state = Some(JobState::Done(code));
                                job_id_and_cmd = Some((job.id, job.command.clone()));
                            }
                            ProcessStatus::Stopped(sig) => {
                                job.state = JobState::Stopped(sig);
                                new_state = Some(JobState::Stopped(sig));
                                job_id_and_cmd = Some((job.id, job.command.clone()));
                            }
                            ProcessStatus::Continued => {
                                job.state = JobState::Running;
                            }
                            ProcessStatus::StillRunning => {}
                        }
                    }

                    if let (Some((id, cmd)), Some(state)) = (job_id_and_cmd, new_state) {
                        notifications.push((id, state, cmd));
                    }
                }
                _ => break,
            }
        }

        // Clean up Done jobs that have been collected for notification
        for (id, state, _) in &notifications {
            if matches!(state, JobState::Done(_)) {
                self.remove_job(*id);
            }
        }

        notifications
    }

    pub fn format_jobs(
        &self,
        flag_l: bool,
        flag_p: bool,
        flag_r: bool,
        flag_s: bool,
    ) -> Vec<String> {
        let mut lines = Vec::new();

        for (&id, job) in &self.jobs {
            let is_running = matches!(job.state, JobState::Running);
            let is_stopped = matches!(job.state, JobState::Stopped(_));

            if flag_r && !is_running {
                continue;
            }
            if flag_s && !is_stopped {
                continue;
            }

            if flag_p {
                lines.push(format!("{}", job.pgid));
                continue;
            }

            let marker = if Some(id) == self.current_job {
                "+"
            } else if Some(id) == self.previous_job {
                "-"
            } else {
                " "
            };

            let status_str = match &job.state {
                JobState::Running => "Running".to_string(),
                JobState::Stopped(sig) => {
                    if *sig == libc::SIGTSTP || *sig == libc::SIGSTOP {
                        "Stopped".to_string()
                    } else {
                        format!("Stopped ({sig})")
                    }
                }
                JobState::Done(code) => {
                    if *code == 0 {
                        "Done".to_string()
                    } else {
                        format!("Exit {code}")
                    }
                }
            };

            if flag_l {
                lines.push(format!(
                    "[{id}]{marker}  {}  {:<12}  {}",
                    job.pgid, status_str, job.command
                ));
            } else {
                lines.push(format!(
                    "[{id}]{marker}  {:<12}  {}",
                    status_str, job.command
                ));
            }
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_manager_add_and_spec_resolution() {
        let mut mgr = JobManager::new();
        assert!(mgr.find_by_spec("%+").is_err());

        let id1 = mgr.add_job(
            1001,
            vec![1001],
            "sleep 10".to_string(),
            JobState::Running,
            None,
        );
        assert_eq!(id1, 1);
        assert_eq!(mgr.current_job, Some(1));
        assert_eq!(mgr.previous_job, None);

        let id2 = mgr.add_job(
            1002,
            vec![1002],
            "cat file.txt".to_string(),
            JobState::Running,
            None,
        );
        assert_eq!(id2, 2);
        assert_eq!(mgr.current_job, Some(2));
        assert_eq!(mgr.previous_job, Some(1));

        // Test resolution
        assert_eq!(mgr.find_by_spec("").unwrap(), 2);
        assert_eq!(mgr.find_by_spec("%+").unwrap(), 2);
        assert_eq!(mgr.find_by_spec("%-").unwrap(), 1);
        assert_eq!(mgr.find_by_spec("%1").unwrap(), 1);
        assert_eq!(mgr.find_by_spec("2").unwrap(), 2);
        assert_eq!(mgr.find_by_spec("%sleep").unwrap(), 1);
        assert_eq!(mgr.find_by_spec("%?file").unwrap(), 2);
        assert!(mgr.find_by_spec("%nonexistent").is_err());
    }

    #[test]
    fn test_job_manager_remove_updates_current_and_previous() {
        let mut mgr = JobManager::new();
        let id1 = mgr.add_job(101, vec![101], "job1".to_string(), JobState::Running, None);
        let id2 = mgr.add_job(102, vec![102], "job2".to_string(), JobState::Running, None);
        let id3 = mgr.add_job(103, vec![103], "job3".to_string(), JobState::Running, None);

        assert_eq!(mgr.current_job, Some(id3));
        assert_eq!(mgr.previous_job, Some(id2));

        // Remove current job (id3)
        mgr.remove_job(id3);
        assert_eq!(mgr.current_job, Some(id2));
        assert_eq!(mgr.previous_job, Some(id1));

        // Remove id2
        mgr.remove_job(id2);
        assert_eq!(mgr.current_job, Some(id1));
        assert_eq!(mgr.previous_job, None);
    }

    #[test]
    fn test_job_formatting() {
        let mut mgr = JobManager::new();
        mgr.add_job(
            2001,
            vec![2001],
            "sleep 10 &".to_string(),
            JobState::Running,
            None,
        );
        mgr.add_job(
            2002,
            vec![2002],
            "vim main.rs".to_string(),
            JobState::Stopped(20),
            None,
        );

        let lines = mgr.format_jobs(false, false, false, false);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("[1]-"));
        assert!(lines[0].contains("Running"));
        assert!(lines[0].contains("sleep 10 &"));

        assert!(lines[1].contains("[2]+"));
        assert!(lines[1].contains("Stopped"));
        assert!(lines[1].contains("vim main.rs"));

        let p_lines = mgr.format_jobs(false, true, false, false);
        assert_eq!(p_lines, vec!["2001", "2002"]);

        let r_lines = mgr.format_jobs(false, false, true, false);
        assert_eq!(r_lines.len(), 1);
        assert!(r_lines[0].contains("sleep 10"));

        let s_lines = mgr.format_jobs(false, false, false, true);
        assert_eq!(s_lines.len(), 1);
        assert!(s_lines[0].contains("vim main.rs"));
    }
}
