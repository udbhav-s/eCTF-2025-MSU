import argparse
import json
import os
import signal
import subprocess
import time
from pathlib import Path
from textual.app import App, ComposeResult
from textual.containers import Container, Horizontal
from textual.widgets import Button, Footer, Header, Static, Input
from textual.binding import Binding

CONFIG_FILE = Path.home() / ".openocd_gdb_tool.json"


class ProcessManager:
    def __init__(self):
        self.openocd_proc = None
        self.gdb_proc = None
        self.openocd_pid = None
        self.gdb_pid = None
        self.screen_session = f"openocd_gdb_{int(time.time())}"  # Unique session name

    def _check_screen_installed(self):
        """Check if screen is installed"""
        try:
            subprocess.run(["which", "screen"], check=True, stdout=subprocess.PIPE)
            return True
        except subprocess.CalledProcessError:
            return False

    def create_screen_session(self):
        """Create a new screen session with two windows"""
        if not self._check_screen_installed():
            raise EnvironmentError(
                "Screen is not installed. Please install it with your package manager."
            )

        # Create a new detached screen session
        subprocess.run(f"screen -dmS {self.screen_session}", shell=True, check=True)

        # Create a second window in the session
        subprocess.run(
            f"screen -S {self.screen_session} -X screen", shell=True, check=True
        )

        # Name the windows
        subprocess.run(
            f"screen -S {self.screen_session} -p 0 -X title openocd", shell=True
        )
        subprocess.run(f"screen -S {self.screen_session} -p 1 -X title gdb", shell=True)

    def launch_openocd(self, cfg):
        """Launch OpenOCD in the first screen window"""
        cmd = f"screen -S {self.screen_session} -p 0 -X stuff 'openocd -f {cfg}\n'"
        subprocess.run(cmd, shell=True)
        # Give OpenOCD time to start
        time.sleep(1)
        return 0  # Can't get actual PID easily here

    def launch_gdb(self, gdb_path, openocd_gdb, elf_file):
        """Launch GDB in the second screen window"""
        cmd = f"screen -S {self.screen_session} -p 1 -X stuff '{gdb_path} -x {openocd_gdb} {elf_file}\n'"
        subprocess.run(cmd, shell=True)
        return 0  # Can't get actual PID easily here

    def attach_to_screen(self):
        """Attach to the screen session"""
        try:
            os.system(f"screen -r {self.screen_session}")
        except Exception as e:
            print(f"Error attaching to screen: {e}")

    def switch_to_window(self, window_name):
        """Switch to a specific window in the screen session"""
        window_num = 0 if window_name == "openocd" else 1
        subprocess.run(
            f"screen -S {self.screen_session} -p {window_num} -X select {window_num}",
            shell=True,
        )

    def terminate(self):
        """Terminate the screen session"""
        try:
            subprocess.run(f"screen -S {self.screen_session} -X quit", shell=True)
        except Exception:
            pass


class DebuggerUI(App):
    CSS = """
    .title {
        text-align: center;
        padding: 1;
    }
    
    .minimal {
        height: 1;
        margin: 0;
        padding: 0;
    }
    
    .status-bar {
        height: 1;
        dock: bottom;
        background: $surface;
    }
    
    .info-text {
        text-align: center;
        padding: 1;
    }
    """

    BINDINGS = [
        Binding("q", "quit", "Quit"),
        Binding("l", "launch", "Launch"),
        Binding("a", "attach", "Attach to Screen"),
        Binding("s", "setup", "Setup"),
    ]

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.defaults = {}
        self.load_defaults()
        self.proc_manager = ProcessManager()
        self.setup_mode = True
        self.screen_created = False

    def load_defaults(self):
        if CONFIG_FILE.exists():
            self.defaults = json.loads(CONFIG_FILE.read_text())

    def save_defaults(self):
        CONFIG_FILE.write_text(json.dumps(self.defaults, indent=4))

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)

        with Container(id="setup_container"):
            yield Static("OpenOCD & GDB TUI Debugger", classes="title")
            yield Input(
                value=self.defaults.get("openocd_cfg", "interface.cfg"),
                placeholder="OpenOCD Config",
                id="openocd_cfg",
            )
            yield Input(
                value=self.defaults.get("gdb_path", "arm-none-eabi-gdb"),
                placeholder="GDB Path",
                id="gdb_path",
            )
            yield Input(
                value=self.defaults.get("openocd_gdb", "./openocd.gdb"),
                placeholder="openocd.gdb file",
                id="openocd_gdb",
            )
            yield Input(
                value=self.defaults.get("elf_file", "./build/output.elf"),
                placeholder="ELF File",
                id="elf_file",
            )
            yield Horizontal(
                Button("Launch [l]", id="launch"),
                Button("Attach to Screen [a]", id="attach"),
                Button("Quit [q]", id="quit"),
            )
            yield Static(
                "Once launched, use Ctrl+a n to switch between windows in screen",
                classes="info-text",
            )
            yield Static(
                "Use Ctrl+a d to detach from screen (return to this UI)",
                classes="info-text",
            )

        # Status bar for when in minimized mode
        yield Static(
            "Press [a] to attach, [s] for setup, [q] to quit",
            id="status_bar",
            classes="status-bar",
        )

        yield Footer()

    def action_attach(self) -> None:
        """Attach to the screen session"""
        if not self.screen_created:
            self.notify("Launch debugger first")
            return

        self.notify("Attaching to screen session...")

        # We need to suspend the textual app to give control to screen
        self.suspend()
        try:
            self.proc_manager.attach_to_screen()
        finally:
            # Resume the textual app after detaching from screen
            self.resume()

    def action_setup(self) -> None:
        """Toggle setup mode"""
        self.setup_mode = not self.setup_mode
        setup = self.query_one("#setup_container")
        status = self.query_one("#status_bar")

        if self.setup_mode:
            setup.remove_class("minimal")
            status.add_class("minimal")
        else:
            setup.add_class("minimal")
            status.remove_class("minimal")

    def action_launch(self) -> None:
        """Launch the debugger in screen"""
        cfg = self.query_one("#openocd_cfg").value
        gdb_path = self.query_one("#gdb_path").value
        openocd_gdb = self.query_one("#openocd_gdb").value
        elf_file = self.query_one("#elf_file").value

        self.defaults = {
            "openocd_cfg": cfg,
            "gdb_path": gdb_path,
            "openocd_gdb": openocd_gdb,
            "elf_file": elf_file,
        }
        self.save_defaults()

        try:
            # Create the screen session first
            self.proc_manager.create_screen_session()
            self.screen_created = True
            self.notify("Screen session created")

            # Launch OpenOCD in the first window
            self.proc_manager.launch_openocd(cfg)
            self.notify("OpenOCD launched in screen window 0")

            # Launch GDB in the second window
            self.proc_manager.launch_gdb(gdb_path, openocd_gdb, elf_file)
            self.notify("GDB launched in screen window 1")

            # Attach to the screen session
            self.notify("Attaching to screen session...")
            self.suspend()
            try:
                self.proc_manager.attach_to_screen()
            finally:
                self.resume()

        except Exception as e:
            self.notify(f"Error: {str(e)}")

    def action_quit(self) -> None:
        """Quit the application"""
        self.exit()

    def on_button_pressed(self, event: Button.Pressed) -> None:
        button_id = event.button.id
        if button_id == "launch":
            self.action_launch()
        elif button_id == "attach":
            self.action_attach()
        elif button_id == "quit":
            self.action_quit()

    def on_mount(self) -> None:
        """Called when app is mounted"""
        # Start in setup mode
        self.setup_mode = True

    def on_unmount(self) -> None:
        """Clean up processes when app is closed"""
        self.proc_manager.terminate()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="OpenOCD & GDB TUI Tool")
    args = parser.parse_args()

    # Create and run the app
    app = DebuggerUI()
    app.run()
