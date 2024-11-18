#!/usr/bin/env python3
import time
from typing import List, Mapping, Optional
from plin.device import PLIN
from plin.structs import PLINMessage
from plin.enums import (
    PLINMode,
    PLINFrameErrorFlag,
    PLINFrameDirection,
    PLINFrameChecksumType,
    PLINFrameFlag,
)
import os
import ldfparser
from dataclasses import dataclass
import argparse
import cmd2


SCHEDULER_SLOT = 0
ALL_BOARDS = [0, 1, 2]


class Frame:
    def __init__(self, plin: PLIN, frame: ldfparser.LinUnconditionalFrame):
        self.plin = plin
        self.frame = frame
        self.signals = {}

        data = frame.encode({})
        self.plin.set_frame_entry(
            frame.frame_id,
            PLINFrameDirection.PUBLISHER,
            PLINFrameChecksumType.ENHANCED,
            PLINFrameFlag.NONE,
            data,
            len(data),
        )
        self.plin.add_unconditional_schedule_slot(SCHEDULER_SLOT, 100, frame.frame_id)

    def update(self, data):
        self.signals.update(data)
        encoded = self.frame.encode(self.signals)
        self.plin.set_frame_entry_data(self.frame.frame_id, 0, encoded, len(encoded))


class Task:
    def update(self):
        pass


@dataclass
class TaskDescriptor:
    period_ms: int
    task: Task
    next_schedule_at: int = 0


class TaskScheduler:
    def __init__(self):
        self.tasks: List[TaskDescriptor] = []

    def add(self, period_ms: int, task: Task):
        self.tasks.append(TaskDescriptor(period_ms, task))

    def process(self):
        now = time.time()

        for task in self.tasks:
            if task.next_schedule_at < now:
                task.next_schedule_at = now + task.period_ms / 1000
                task.task.update()


class SnakeLedsTask(Task):
    def __init__(self, scheduler: "Scheduler", board_id: int):
        self.frame = scheduler.add_master_frame(f"eval_{board_id}_leds")
        self.board_id = board_id
        self.pos = 0
        self.leds = [0] * 4

    def update(self):
        self.leds[self.pos] = not self.leds[self.pos]
        self.pos = (self.pos + 1) % len(self.leds)

        self.frame.update(
            {
                f"eval_{self.board_id}_led{led}": state
                for led, state in enumerate(self.leds)
            }
        )


class RGBTask(Task):
    def __init__(self, scheduler: "Scheduler", board_id: int):
        self.frame = scheduler.add_master_frame(f"eval_{board_id}_rgb")
        self.board_id = board_id
        self.color = [0] * 3
        self.ch = 0

    def update(self):
        self.color[self.ch] += 30
        if self.color[self.ch] > 255:
            self.color[self.ch] = 0
            self.ch = (self.ch + 1) % 3

        self.frame.update(
            {
                f"eval_{self.board_id}_rgb_r": self.color[0],
                f"eval_{self.board_id}_rgb_g": self.color[1],
                f"eval_{self.board_id}_rgb_b": self.color[2],
            }
        )


class Scheduler:
    def __init__(self, boards: List[int]):
        self.db = ldfparser.parse_ldf("lin_eval.ldf")
        self.rx_frames: Mapping[int, ldfparser.LinUnconditionalFrame] = {}

        self.plin = PLIN(interface="/dev/plin0")
        self.plin.start(mode=PLINMode.MASTER, baudrate=19200)
        self.plin.set_id_filter(bytearray([0xFF] * 8))

    def add_master_frame(self, name: str) -> Frame:
        return Frame(self.plin, self.db.get_frame(name))

    def add_slave_frame(self, name: str):
        frame = self.db.get_frame(name)
        self.plin.set_frame_entry(
            frame.frame_id,
            PLINFrameDirection.SUBSCRIBER_AUTO_LEN,
            PLINFrameChecksumType.ENHANCED,
            PLINFrameFlag.NONE,
        )
        self.plin.add_unconditional_schedule_slot(SCHEDULER_SLOT, 100, frame.frame_id)
        self.rx_frames[frame.frame_id] = frame

    def start(self):
        self.plin.start_schedule(SCHEDULER_SLOT)

    def read(self) -> Optional[PLINMessage]:
        result = os.read(self.plin.fd, PLINMessage.buffer_length)
        return PLINMessage.from_buffer_copy(result)


class Demo:
    def __init__(self, scheduler: Scheduler):
        self.scheduler = scheduler
        self.tasks = TaskScheduler()

    def run(self, boards: List[int]):
        for board in boards:
            self.tasks.add(10, RGBTask(self.scheduler, board))
            self.tasks.add(200, SnakeLedsTask(self.scheduler, board))
            self.scheduler.add_slave_frame(f"eval_{board}_photores")

        self.scheduler.start()
        while True:
            self.tasks.process()

            frame = self.scheduler.read()
            if frame:
                print(f"{frame.id: 2x} ", end="")
                if frame.flags:
                    print(PLINFrameErrorFlag(frame.flags))
                else:
                    data = bytearray(frame.data[: frame.len])
                    print(f"[{frame.len}] {data.hex(' ')}")

                    frame_db = self.scheduler.rx_frames.get(frame.id, None)
                    if frame_db:
                        print(frame_db.decode(data))


class LinCli(cmd2.Cmd):
    prompt = "lin> "

    def __init__(self, scheduler: Scheduler):
        super().__init__(
            allow_cli_args=False, persistent_history_file="~/.config/lin_eval_history"
        )
        self.scheduler = scheduler

        self.frames: Mapping[str, Frame] = {}
        for frame in scheduler.db.get_unconditional_frames():
            if isinstance(frame.publisher, ldfparser.node.LinMaster):
                self.frames[frame.name] = scheduler.add_master_frame(frame.name)
            else:
                scheduler.add_slave_frame(frame.name)

        self.scheduler.start()

    rgb_parser = cmd2.Cmd2ArgumentParser()
    rgb_parser.add_argument("board_id", type=int)
    rgb_parser.add_argument("r", type=int)
    rgb_parser.add_argument("g", type=int)
    rgb_parser.add_argument("b", type=int)

    @cmd2.with_argparser(rgb_parser)
    def do_rgb(self, args):
        prefix = f"eval_{args.board_id}_rgb"

        self.frames[prefix].update(
            {
                f"{prefix}_r": args.r,
                f"{prefix}_g": args.g,
                f"{prefix}_b": args.b,
            }
        )

    led_parser = cmd2.Cmd2ArgumentParser()
    led_parser.add_argument("board_id", type=int)
    led_parser.add_argument("led", type=int)
    led_parser.add_argument("state", type=int)

    @cmd2.with_argparser(led_parser)
    def do_led(self, args):
        prefix = f"eval_{args.board_id}_led"

        self.frames[f"{prefix}s"].update(
            {
                f"{prefix}{args.led}": args.state,
            }
        )

    off_parser = cmd2.Cmd2ArgumentParser()
    off_parser.add_argument("board_id", type=int, nargs="?")

    @cmd2.with_argparser(off_parser)
    def do_off(self, args):
        boards = [args.board_id] if args.board_id else ALL_BOARDS
        for board_id in boards:
            self.frames[f"eval_{board_id}_rgb"].update(
                {
                    f"eval_{board_id}_rgb_r": 0,
                    f"eval_{board_id}_rgb_g": 0,
                    f"eval_{board_id}_rgb_b": 0,
                }
            )
            self.frames[f"eval_{board_id}_leds"].update(
                {
                    f"eval_{board_id}_led0": 0,
                    f"eval_{board_id}_led1": 0,
                    f"eval_{board_id}_led2": 0,
                    f"eval_{board_id}_led3": 0,
                }
            )

    def do_monitor(self, args):
        while True:
            frame = self.scheduler.read()
            if frame:
                if frame.flags:
                    print(PLINFrameErrorFlag(frame.flags))
                else:
                    data = bytearray(frame.data[: frame.len])
                    frame_db = self.scheduler.rx_frames.get(frame.id, None)
                    if frame_db:
                        print(frame_db.name, frame_db.decode(data))


parser = argparse.ArgumentParser()
parser.add_argument(
    "--boards", "-b", default=ALL_BOARDS, type=lambda s: [int(b) for b in s.split(",")]
)
parser.add_argument("command", nargs="?", choices=["demo", "shell"], default="demo")
args = parser.parse_args()

scheduler = Scheduler(args.boards)

if args.command == "demo":
    Demo(scheduler).run(args.boards)
elif args.command == "shell":
    cli = LinCli(scheduler)
    cli.cmdloop()
