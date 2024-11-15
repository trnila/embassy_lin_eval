#!/usr/bin/env python3
import time
from typing import List, Mapping
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


SCHEDULER_SLOT = 0


class Frame:
    def __init__(self, plin: PLIN, frame: ldfparser.LinUnconditionalFrame):
        self.plin = plin
        self.frame = frame

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
        encoded = self.frame.encode(data)
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

        self.tasks = TaskScheduler()
        for board in boards:
            self.tasks.add(10, RGBTask(self, board))
            self.tasks.add(200, SnakeLedsTask(self, board))

        self.add_slave_frame("eval_0_photores")

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

    def run(self):
        self.plin.start_schedule(SCHEDULER_SLOT)

        while True:
            self.tasks.process()

            result = os.read(self.plin.fd, PLINMessage.buffer_length)
            frame = PLINMessage.from_buffer_copy(result)
            # frame = plin.read()
            if frame:
                print(f"{frame.id: 2x} ", end="")
                if frame.flags:
                    print(PLINFrameErrorFlag(frame.flags))
                else:
                    data = bytearray(frame.data[: frame.len])
                    print(f"[{frame.len}] {data.hex(' ')}")

                    frame_db = self.rx_frames.get(frame.id, None)
                    if frame_db:
                        print(frame_db.decode(data))


parser = argparse.ArgumentParser()
parser.add_argument(
    "--boards", "-b", default=[0, 1, 2], type=lambda s: [int(b) for b in s.split(",")]
)
args = parser.parse_args()

Scheduler(args.boards).run()
