// 应用入口：桌面发行版隐藏控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    maiduo_cuotiben_lib::run();
}
