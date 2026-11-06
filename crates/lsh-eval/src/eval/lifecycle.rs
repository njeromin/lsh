use std::{
    io::{self},
    path::Path,
    sync::Arc,
};

#[cfg(unix)]
use lsh_sys::signals::{
    ProcessStatus, SIGINT, TerminalForegroundGuard, get_terminal_attributes, is_interactive_tty,
    restore_shell_termios, signal_exit_code, wait_pid_status,
};

use crate::{
    eval::{EvalFlow, Evaluator, value::Value},
    io::IoCtx,
    jobs::JobState,
};

impl Evaluator {
    pub fn trigger_pre_prompt(&mut self) {
        let registry_hooks = self.hook_registry.read().unwrap().get_hooks("pre_prompt");
        for func in registry_hooks {
            let mut scope_indices = vec![0];
            self.call_user_function_with_values_and_receiver(
                &func,
                vec![],
                None,
                &mut scope_indices,
            );
        }
    }

    pub fn trigger_pre_exec(&mut self, cmd_line: &str) {
        let registry_hooks = self.hook_registry.read().unwrap().get_hooks("pre_exec");
        for func in registry_hooks {
            let mut scope_indices = vec![0];
            self.call_user_function_with_values_and_receiver(
                &func,
                vec![Value::String(cmd_line.to_string())],
                None,
                &mut scope_indices,
            );
        }
    }

    pub fn trigger_post_exec(&mut self, cmd_line: &str, exit_code: i32) {
        let registry_hooks = self.hook_registry.read().unwrap().get_hooks("post_exec");
        for func in registry_hooks {
            let mut scope_indices = vec![0];
            self.call_user_function_with_values_and_receiver(
                &func,
                vec![
                    Value::String(cmd_line.to_string()),
                    Value::Number(exit_code as f64),
                ],
                None,
                &mut scope_indices,
            );
        }
    }

    pub fn trigger_chpwd(&mut self, old_pwd: &str, new_pwd: &str) {
        let registry_hooks = self.hook_registry.read().unwrap().get_hooks("chpwd");
        for func in registry_hooks {
            let mut scope_indices = vec![0];
            self.call_user_function_with_values_and_receiver(
                &func,
                vec![
                    Value::String(old_pwd.to_string()),
                    Value::String(new_pwd.to_string()),
                ],
                None,
                &mut scope_indices,
            );
        }
    }

    pub fn reap_and_notify_jobs<W: io::Write>(&mut self, out: &mut W) {
        let notifications = self.job_manager.reap_jobs();
        for (id, state, cmd) in notifications {
            match state {
                JobState::Done(code) => {
                    if code == 0 {
                        let _ = writeln!(out, "[{id}]+  Done                    {cmd}");
                    } else {
                        let _ = writeln!(out, "[{id}]+  Exit {code:<18} {cmd}");
                    }
                }
                JobState::Stopped(sig) => {
                    let _ = writeln!(out, "[{id}]+  Stopped ({sig})         {cmd}");
                }
                _ => {}
            }
        }
    }

    pub fn resume_foreground_job<R: io::Read, W: io::Write, WE: io::Write>(
        &mut self,
        job_id: usize,
        ctx: &mut IoCtx<R, W, WE>,
    ) -> i32 {
        let Some(job) = self.job_manager.get_job(job_id) else {
            let _ = writeln!(ctx.err, "fg: job {job_id} not found");
            return 1;
        };

        let pgid = job.pgid;
        let pids = job.pids.clone();
        let cmd = job.command.clone();
        #[cfg(unix)]
        let saved_termios = job.saved_termios;

        let _ = writeln!(ctx.out, "{cmd}");

        #[cfg(unix)]
        {
            if let Some(tmodes) = saved_termios {
                lsh_sys::set_terminal_attributes(&tmodes);
            }

            let _guard = if is_interactive_tty() {
                TerminalForegroundGuard::give_to_pgid(pgid)
            } else {
                TerminalForegroundGuard::inactive()
            };

            unsafe {
                libc::killpg(pgid, libc::SIGCONT);
            }

            if let Some(j) = self.job_manager.get_job_mut(job_id) {
                j.state = JobState::Running;
            }

            let mut remaining_pids = pids;
            let mut last_code = 0;

            loop {
                if remaining_pids.is_empty() {
                    break;
                }

                match wait_pid_status(-pgid, libc::WUNTRACED) {
                    Ok((_pid, ProcessStatus::Stopped(sig))) => {
                        let new_tmodes = get_terminal_attributes();
                        restore_shell_termios();
                        drop(_guard);
                        if let Some(j) = self.job_manager.get_job_mut(job_id) {
                            j.state = JobState::Stopped(sig);
                            j.saved_termios = new_tmodes;
                        }
                        let _ = writeln!(ctx.err, "\n[{job_id}]+  Stopped                 {cmd}");
                        return signal_exit_code(sig);
                    }
                    Ok((pid, ProcessStatus::Exited(code))) => {
                        remaining_pids.retain(|&p| p != pid);
                        last_code = code;
                    }
                    Ok((pid, ProcessStatus::Signaled(sig))) => {
                        remaining_pids.retain(|&p| p != pid);
                        if sig == SIGINT {
                            let _ = writeln!(ctx.err);
                        }
                        last_code = signal_exit_code(sig);
                    }
                    Ok((_, ProcessStatus::StillRunning)) | Ok((_, ProcessStatus::Continued)) => {
                        continue;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }

            restore_shell_termios();
            self.job_manager.remove_job(job_id);
            last_code
        }
        #[cfg(not(unix))]
        {
            0
        }
    }

    pub fn import_module(&mut self, name_or_path: &str) -> Result<Value, String> {
        let path = self
            .plugin_manager
            .find_plugin_file(name_or_path)
            .ok_or_else(|| {
                format!("Module or plugin '{name_or_path}' not found in search paths.")
            })?;

        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());

        {
            let cache = self.module_cache.read().unwrap();
            if let Some(cached_val) = cache.get(&canonical_path) {
                return Ok(cached_val.clone());
            }
        }

        let scope_index = self.make_scope();
        let mut scope_indices = vec![0, scope_index];
        let outcome_res =
            crate::eval::exec::exec_file_outcome_scoped(self, path, &mut scope_indices);

        if scope_indices.pop() == Some(scope_index) {
            self.scopes.pop();
        }

        let outcome = outcome_res?;
        let export_val = match outcome.flow {
            EvalFlow::Return(v) => v,
            _ => Value::Null,
        };

        if let Value::Object(ref map) = export_val {
            // Flatten prompt_segments so plugin methods (like .git) can be accessed directly on manifest
            if let Some(Value::Object(seg_map)) = export_val
                .get_property("prompt_segments")
                .or_else(|| export_val.get_property("segments"))
            {
                let seg_guard = seg_map.read().unwrap_or_else(|p| p.into_inner()).clone();
                let mut m_guard = map.write().unwrap_or_else(|p| p.into_inner());
                for (k, v) in seg_guard {
                    m_guard.entry(k).or_insert(v);
                }
            }

            // Register hooks in hook_registry
            let mut hooks_to_register = Vec::new();
            if let Some(Value::Object(hooks_map)) = export_val.get_property("hooks") {
                let h_guard = hooks_map.read().unwrap_or_else(|p| p.into_inner()).clone();
                for (event_name, hook_val) in h_guard {
                    if let Value::Function(f) = hook_val {
                        let bound = Arc::new(f.with_bound_this(export_val.clone()));
                        hooks_to_register.push((event_name, bound));
                    }
                }
            }

            let top_level_hook_candidates = [
                "on_chpwd",
                "chpwd",
                "on_pre_prompt",
                "pre_prompt",
                "on_pre_exec",
                "pre_exec",
                "on_post_exec",
                "post_exec",
            ];
            for hook_name in top_level_hook_candidates {
                if let Some(Value::Function(f)) = export_val.get_property(hook_name) {
                    let bound = Arc::new(f.with_bound_this(export_val.clone()));
                    hooks_to_register.push((hook_name.to_string(), bound));
                }
            }

            if !hooks_to_register.is_empty() {
                let mut reg = self.hook_registry.write().unwrap();
                for (event, func) in hooks_to_register {
                    reg.add(&event, func);
                }
            }

            // Register in plugin_manager if manifest has a non-empty name
            if let Some(Value::String(ref name)) = export_val.get_property("name")
                && !name.trim().is_empty()
            {
                let _ = self.plugin_manager.register(export_val.clone());
            }
        }

        self.module_cache
            .write()
            .unwrap()
            .insert(canonical_path, export_val.clone());

        Ok(export_val)
    }

    pub fn load_plugin(&mut self, name_or_path: &str) -> Result<String, String> {
        let manifest_val = self.import_module(name_or_path)?;
        let Value::Object(_) = manifest_val else {
            return Err(format!(
                "Plugin '{name_or_path}' must return an object manifest (e.g. 'return {{ name = \"...\", ... }}')."
            ));
        };
        let default_name = Path::new(name_or_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name_or_path);
        let name = self
            .plugin_manager
            .register_with_default_name(default_name, manifest_val)?;
        Ok(name)
    }

    pub fn unload_plugin(&mut self, name: &str) -> bool {
        self.on_plugin_unregistered(name);
        if let Some(plugin) = self.plugin_manager.get(name) {
            let manifest = plugin.manifest.clone();
            let candidate_hooks = ["pre_prompt", "pre_exec", "post_exec", "chpwd"];
            for hook in candidate_hooks {
                let full = format!("on_{hook}");
                let func_to_remove = manifest
                    .get_property("hooks")
                    .and_then(|h| h.get_property(&full).or_else(|| h.get_property(hook)))
                    .or_else(|| manifest.get_property(&full))
                    .or_else(|| manifest.get_property(hook));
                if let Some(Value::Function(f)) = func_to_remove {
                    self.hook_registry.write().unwrap().remove(hook, &f);
                }
            }
        }
        self.plugin_manager.unregister(name)
    }

    pub fn on_plugin_registered(&mut self, _plugin_name: &str) {}

    pub fn on_plugin_unregistered(&mut self, plugin_name: &str) {
        let segment_names = self.plugin_manager.list_prompt_segments_for(plugin_name);
        if segment_names.is_empty() {
            return;
        }

        let Some(prompt_val) = self.get_var(0, "prompt").cloned() else {
            return;
        };

        let Value::Object(ref prompt_map) = prompt_val else {
            return;
        };

        let mut prompt_guard = prompt_map.write().unwrap_or_else(|p| p.into_inner());

        if let Some(Value::Array(arr)) = prompt_guard.get("segments") {
            if let Ok(mut a) = arr.write() {
                a.retain(|v| match v {
                    Value::Function(f) => {
                        if let Some(ref bound) = f.bound_this
                            && let Some(Value::String(name)) = bound.get_property("name")
                        {
                            return name != plugin_name;
                        }
                        true
                    }
                    Value::String(s) => !segment_names.contains(s),
                    _ => true,
                });
            }
        } else if let Some(Value::Object(seg_map)) = prompt_guard.get("segments") {
            let mut seg_guard = seg_map.write().unwrap_or_else(|p| p.into_inner());
            for seg_name in &segment_names {
                seg_guard.remove(seg_name);
            }
        }

        match prompt_guard.get_mut("order") {
            Some(Value::Array(arr)) => {
                if let Ok(mut a) = arr.write() {
                    a.retain(|v| match v {
                        Value::String(s) => !segment_names.contains(s),
                        _ => true,
                    });
                }
            }
            Some(Value::String(order_str)) => {
                let order_parts: Vec<String> = order_str
                    .split_whitespace()
                    .filter(|part| !segment_names.iter().any(|seg| seg == part))
                    .map(|s| s.to_string())
                    .collect();

                *order_str = order_parts.join(" ");
            }
            _ => {}
        }
    }
}
