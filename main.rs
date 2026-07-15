use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use winreg::enums::*;
use winreg::RegKey;
use sysinfo::System;

const AFTER_REBOOT_VAR: &str = "ADSK_NLM_AFTER_REBOOT";
const REBOOT_TIMESTAMP_VAR: &str = "ADSK_NLM_REBOOT_TIMESTAMP";
const RUNONCE_KEY: &str = "AdskNLMUpdate";

fn main() -> ExitCode {
    
    println!("Проверка прав администратора...");
    if !is_admin() {
        eprintln!("Скрипт должен запускаться от имени администратора!");
        println!();
        println!("Нажмите Enter для выхода...");
        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();
        return ExitCode::from(1);
    }
    println!("Права администратора подтверждены");
    println!();
    let after_reboot_flag = env::var(AFTER_REBOOT_VAR).unwrap_or_default();
    let was_reboot = check_if_rebooted();
    
    println!("Режим запуска: {}", 
        if after_reboot_flag == "1" && was_reboot { 
            "после перезагрузки" 
        } else if after_reboot_flag == "1" && !was_reboot {
            "обнаружена старая метка (перезагрузки не было)"
        } else {
            "первичный запуск"
        });
    
    if after_reboot_flag == "1" && was_reboot {
        println!("Обнаружен флаг продолжения после перезагрузки");
        cleanup_reboot_flags();
        
        println!();
        println!("Продолжение после перезагрузки ");
        println!();
        run_after_reboot()
    } else {
        if after_reboot_flag == "1" {
            println!("[ДЕЙСТВИЕ] Очистка старой метки перезагрузки...");
            cleanup_reboot_flags();
            println!();
        }
        
        println!();
        println!(" Первичный запуск ");
        println!();
        run_before_reboot()
    }
}

fn get_system_uptime() -> Result<u64, String> {
    let system = System::new_all();
    let uptime = System::uptime();
    Ok(uptime)
}

fn check_if_rebooted() -> bool {
    if let Ok(timestamp_str) = env::var(REBOOT_TIMESTAMP_VAR) {
        if let Ok(saved_time) = timestamp_str.parse::<u64>() {
            if let Ok(uptime) = get_system_uptime() {
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                let boot_time = current_time - uptime;
                
                println!("[ПРОВЕРКА] Время последней метки: {}", 
                    format_timestamp(saved_time));
                println!("[ПРОВЕРКА] Время загрузки системы: {}", 
                    format_timestamp(boot_time));
                
                if boot_time > saved_time {
                    println!("[OK] Подтверждено: система была перезагружена");
                    return true;
                } else {
                    println!("[ИНФО] Система не перезагружалась после установки метки");
                    return false;
                }
            }
        }
    }
    
    if env::var(AFTER_REBOOT_VAR).unwrap_or_default() == "1" {
        if let Ok(uptime) = get_system_uptime() {
            if uptime < 300 {
                println!("[ПРОВЕРКА] Система работает менее 5 минут - вероятно была перезагрузка");
                return true;
            }
        }
        println!("[ПРОВЕРКА] Не удалось подтвердить перезагрузку");
        return false;
    }
    
    false
}

fn format_timestamp(secs: u64) -> String {
    if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
        let now = duration.as_secs();
        if secs > now {
            return "будущее".to_string();
        }
        let diff = now - secs;
        if diff < 60 {
            format!("{} сек назад", diff)
        } else if diff < 3600 {
            format!("{} мин назад", diff / 60)
        } else if diff < 86400 {
            format!("{} ч назад", diff / 3600)
        } else {
            format!("{} дн назад", diff / 86400)
        }
    } else {
        secs.to_string()
    }
}

fn cleanup_reboot_flags() {
    println!("[ДЕЙСТВИЕ] Очистка флагов перезагрузки...");
    env::remove_var(AFTER_REBOOT_VAR);
    env::remove_var(REBOOT_TIMESTAMP_VAR);
    remove_permanent_env_var(AFTER_REBOOT_VAR);
    remove_permanent_env_var(REBOOT_TIMESTAMP_VAR);
    remove_runonce_entry();
    println!("[OK] Флаги очищены");
}

fn is_admin() -> bool {
    let test_path = r"C:\Program Files (x86)\Common Files\Autodesk Shared\admin_test.tmp";
    match fs::write(test_path, "test") {
        Ok(_) => {
            fs::remove_file(test_path).ok();
            true
        }
        Err(_) => false,
    }
}

fn run_before_reboot() -> ExitCode {
    stop_process("AdskNLM.exe");
    println!();
    
    println!("Шаг 2 из 6: Запуск деинсталлятора ");
    let uninstall_path = r"C:\Program Files (x86)\Common Files\Autodesk Shared\AdskLicensing\uninstall.exe";
    println!("[ПУТЬ] {}", uninstall_path);
    
    if Path::new(uninstall_path).exists() {
        println!("[ЗАПУСК] Запуск деинсталлятора...");
        match Command::new(uninstall_path)
            .status()
        {
            Ok(status) => {
                let code = status.code().unwrap_or(-1);
                println!("[РЕЗУЛЬТАТ] Деинсталляция завершена с кодом: {}", code);
                if status.success() {
                    println!("[OK] Деинсталляция успешна");
                } else {
                    println!("[ПРЕДУПРЕЖДЕНИЕ] Деинсталляция завершилась с ошибкой (код: {})", code);
                }
            }
            Err(e) => {
                eprintln!("[ОШИБКА] Не удалось запустить деинсталлятор: {}", e);
            }
        }
    } else {
        println!("[ПРЕДУПРЕЖДЕНИЕ] uninstall.exe не найден, пропускаем шаг");
    }
    println!("");
    println!();
    
    println!("Шаг 3 из 6: Очистка папки Network License Manager ");
    let nlm_folder = r"C:\Program Files (x86)\Common Files\Autodesk Shared\Network License Manager";
    println!("[ПУТЬ] {}", nlm_folder);
    
    if Path::new(nlm_folder).exists() {
        println!("[ДЕЙСТВИЕ] Удаление папки...");
        match fs::remove_dir_all(nlm_folder) {
            Ok(_) => {
                println!("[OK] Папка успешно удалена");
                println!("[ДЕЙСТВИЕ] Создание пустой папки...");
                match fs::create_dir_all(nlm_folder) {
                    Ok(_) => println!("[OK] Папка пересоздана"),
                    Err(e) => eprintln!("[ОШИБКА] Не удалось создать папку: {}", e),
                }
            }
            Err(e) => {
                eprintln!("[ОШИБКА] Не удалось удалить папку: {}", e);
                println!("[ДЕЙСТВИЕ] Попытка очистки содержимого...");
                clean_directory(nlm_folder);
                println!("[OK] Содержимое папки очищено (частично)");
            }
        }
    } else {
        println!("[ИНФО] Папка не существует");
        println!("[ДЕЙСТВИЕ] Создание папки...");
        match fs::create_dir_all(nlm_folder) {
            Ok(_) => println!("[OK] Папка создана"),
            Err(e) => eprintln!("[ОШИБКА] Не удалось создать папку: {}", e),
        }
    }
    println!("");
    println!();
    
    println!("Шаг 4 из 6: Очистка временных файлов ");
    if let Ok(temp) = env::var("TEMP") {
        println!("[ПУТЬ] {}", temp);
        println!("[ДЕЙСТВИЕ] Удаление временных файлов...");
        
        let mut deleted_count = 0;
        let mut error_count = 0;
        
        if let Ok(entries) = fs::read_dir(&temp) {
            for entry in entries.flatten() {
                let path = entry.path();
                let result = if path.is_dir() {
                    fs::remove_dir_all(&path)
                } else {
                    fs::remove_file(&path)
                };
                
                match result {
                    Ok(_) => deleted_count += 1,
                    Err(_) => error_count += 1,
                }
            }
        }
        
        println!("[РЕЗУЛЬТАТ] Удалено: {}, ошибок: {}", deleted_count, error_count);
        if error_count > 0 {
            println!("[ИНФО] Некоторые файлы не удалось удалить (заняты системой)");
        }
    } else {
        println!("[ПРЕДУПРЕЖДЕНИЕ] Переменная TEMP не найдена");
    }
    println!("");
    println!();
    
    println!("Шаг 5 из 6: Подготовка к перезагрузке ");
    
    println!("[ДЕЙСТВИЕ] Установка флагов продолжения...");
    env::set_var(AFTER_REBOOT_VAR, "1");
    set_permanent_env_var(AFTER_REBOOT_VAR, "1");
    
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let timestamp_str = current_time.to_string();
    env::set_var(REBOOT_TIMESTAMP_VAR, &timestamp_str);
    set_permanent_env_var(REBOOT_TIMESTAMP_VAR, &timestamp_str);
    
    println!("[OK] Флаги установлены (timestamp: {})", current_time);
    
    println!("[ДЕЙСТВИЕ] Регистрация в автозагрузке...");
    if let Ok(exe_path) = env::current_exe() {
        println!("[ПУТЬ] {}", exe_path.display());
        let command = format!(
            r#"powershell.exe -ExecutionPolicy Bypass -WindowStyle Normal -Command "Start-Process '{}' -Verb RunAs""#,
            exe_path.display()
        );
        set_runonce_entry(&command);
        println!("[OK] Запись в RunOnce добавлена");
    } else {
        eprintln!("[ОШИБКА] Не удалось получить путь к исполняемому файлу");
    }
    println!("");
    println!();
    
    println!("Шаг 6 из 6: Перезагрузка компьютера ");
    println!("[ИНФО] Компьютер будет перезагружен через 10 секунд...");
    println!("[ИНФО] После перезагрузки скрипт продолжит выполнение автоматически");
    
    for i in (1..=10).rev() {
        print!("\r[ОТСЧЕТ] Перезагрузка через {} секунд...", i);
        io::stdout().flush().ok();
        thread::sleep(Duration::from_secs(1));
    }
    println!();
    println!("[ДЕЙСТВИЕ] Выполнение перезагрузки...");
    
    if let Err(e) = Command::new("shutdown")
        .args(["/r", "/f", "/t", "0"])
        .status()
    {
        eprintln!("[ОШИБКА] Не удалось выполнить перезагрузку: {}", e);
        println!("[РУЧНОЙ ЗАПУСК] Перезагрузите компьютер вручную и запустите программу снова");
        
        cleanup_reboot_flags();
        
        println!();
        println!("Нажмите Enter для выхода...");
        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();
        return ExitCode::from(1);
    }
    println!("");
    
    ExitCode::from(0)
}

fn run_after_reboot() -> ExitCode {
    println!("Шаг 7 из 8: Запуск установщика ");
    let installer_name = "AdskLicensing-installer.exe";
    let current_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    
    println!("[ДИРЕКТОРИЯ] {}", current_dir.display());
    let installer_path = current_dir.join(installer_name);
    println!("[ПУТЬ] {}", installer_path.display());
    
    if installer_path.exists() {
        println!("[ЗАПУСК] Запуск установщика...");
        match Command::new(&installer_path)
            .status()
        {
            Ok(status) => {
                let code = status.code().unwrap_or(-1);
                println!("[РЕЗУЛЬТАТ] Установщик завершен с кодом: {}", code);
                if status.success() {
                    println!("[OK] Установка успешна");
                } else {
                    println!("[ПРЕДУПРЕЖДЕНИЕ] Установщик завершился с ошибкой (код: {})", code);
                    println!("[ПРОВЕРКА] Проверяем, был ли установлен AdskNLM.exe...");
                    
                    let nlm_installed_path = r"C:\Program Files (x86)\Common Files\Autodesk Shared\Network License Manager\AdskNLM.exe";
                    if Path::new(nlm_installed_path).exists() {
                        println!("[OK] Несмотря на код ошибки, AdskNLM.exe был установлен");
                    }
                }
            }
            Err(e) => {
                eprintln!("[ОШИБКА] Не удалось запустить установщик: {}", e);
            }
        }
    } else {
        eprintln!("[ОШИБКА] AdskLicensing-installer.exe не найден!");
        eprintln!("[ПОДСКАЗКА] Поместите файл в папку: {}", current_dir.display());
    }
    println!("");
    println!();
    
    println!("Шаг 8 из 8: Запуск Network License Manager ");
    let nlm_name = "AdskNLM.exe";
    let nlm_path = current_dir.join(nlm_name);
    println!("[ПУТЬ] {}", nlm_path.display());
    
    if nlm_path.exists() {
        println!("[ЗАПУСК] Запуск AdskNLM.exe (серверный процесс)...");
        println!("[ИНФО] Процесс будет запущен в фоновом режиме");
        
        match Command::new(&nlm_path)
            .spawn()
        {
            Ok(child) => {
                println!("[OK] AdskNLM.exe запущен (PID: {})", child.id());
                println!("[ИНФО] Процесс продолжит работу в фоне");
                
                println!("[ОЖИДАНИЕ] Ожидание инициализации (3 секунды)...");
                thread::sleep(Duration::from_secs(3));
                
                let mut system = System::new_all();
                system.refresh_all();
                let is_running = system
                    .processes()
                    .iter()
                    .any(|(_, process)| process.name() == nlm_name);
                
                if is_running {
                    println!("[OK] AdskNLM.exe успешно работает в фоне");
                } else {
                    println!("[ПРЕДУПРЕЖДЕНИЕ] AdskNLM.exe завершился сразу после запуска");
                }
            }
            Err(e) => {
                eprintln!("[ОШИБКА] Не удалось запустить AdskNLM.exe: {}", e);
                eprintln!("[ПРОВЕРКА] Проверьте права доступа и наличие файла");
            }
        }
    } else {
        eprintln!("[ОШИБКА] AdskNLM.exe не найден в папке скрипта!");
        eprintln!("[ПОДСКАЗКА] Поместите файл в папку: {}", current_dir.display());
        
        let system_nlm_path = r"C:\Program Files (x86)\Common Files\Autodesk Shared\Network License Manager\AdskNLM.exe";
        if Path::new(system_nlm_path).exists() {
            println!("[ИНФО] Найден системный AdskNLM.exe: {}", system_nlm_path);
            println!("[ЗАПУСК] Запуск системного AdskNLM.exe...");
            match Command::new(system_nlm_path).spawn() {
                Ok(child) => println!("[OK] Системный AdskNLM.exe запущен (PID: {})", child.id()),
                Err(e) => eprintln!("[ОШИБКА] Не удалось запустить системный AdskNLM.exe: {}", e),
            }
        }
    }
    println!("");
    println!();
    
    println!("");
    println!("          Все операции завершены!                        ");
    println!("");
    println!();
    println!("Нажмите Enter для выхода...");
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    ExitCode::from(0)
}

fn stop_process(name: &str) {
    println!("[ПОИСК] Поиск процесса: {}", name);
    let mut system = System::new_all();
    system.refresh_all();
    
    let pids: Vec<_> = system
        .processes()
        .iter()
        .filter(|(_, process)| process.name() == name)
        .map(|(pid, _)| *pid)
        .collect();
    
    if pids.is_empty() {
        println!("[ИНФО] Процесс {} не запущен", name);
        return;
    }
    
    println!("[НАЙДЕНО] Найдено процессов: {}", pids.len());
    
    for pid in &pids {
        println!("[ДЕЙСТВИЕ] Отправка сигнала завершения PID: {}", pid);
        if let Some(process) = system.process(*pid) {
            if process.kill() {
                println!("[OK] Сигнал отправлен PID: {}", pid);
            } else {
                eprintln!("[ОШИБКА] Не удалось отправить сигнал PID: {}", pid);
            }
        }
    }
    
    println!("[ОЖИДАНИЕ] Ожидание завершения процессов...");
    for i in 0..30 {
        thread::sleep(Duration::from_secs(1));
        system.refresh_all();
        
        let still_running = system
            .processes()
            .iter()
            .filter(|(_, process)| process.name() == name)
            .count();
        
        if i % 5 == 0 {
            println!("[ПРОВЕРКА] {} сек: процессов осталось: {}", i + 1, still_running);
        }
        
        if still_running == 0 {
            println!("[OK] Все процессы {} остановлены за {} сек", name, i + 1);
            return;
        }
    }
    
    eprintln!("[ПРЕДУПРЕЖДЕНИЕ] Не удалось дождаться остановки всех процессов {} за 30 сек", name);
}

fn clean_directory(path: &str) {
    println!("[ОЧИСТКА] Сканирование директории: {}", path);
    let mut count = 0;
    
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let result = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            
            if result.is_ok() {
                count += 1;
            }
        }
    }
    
    println!("[ОЧИСТКА] Удалено элементов: {}", count);
}

fn set_permanent_env_var(name: &str, value: &str) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(env_key) = hkcu.open_subkey_with_flags("Environment", KEY_WRITE) {
        env_key.set_value(name, &value).ok();
    }
}

fn remove_permanent_env_var(name: &str) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(env_key) = hkcu.open_subkey_with_flags("Environment", KEY_WRITE) {
        env_key.delete_value(name).ok();
    }
}

fn set_runonce_entry(command: &str) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(runonce_key) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\RunOnce",
        KEY_WRITE,
    ) {
        runonce_key.set_value(RUNONCE_KEY, &command).ok();
    }
}

fn remove_runonce_entry() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(runonce_key) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\RunOnce",
        KEY_WRITE,
    ) {
        runonce_key.delete_value(RUNONCE_KEY).ok();
    }
}