#!/usr/bin/env python3
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


class Scheduler:
    def __init__(self):
        self.db = ldfparser.parse_ldf("lin_eval.ldf")

        self.plin = PLIN(interface="/dev/plin0")
        self.plin.start(mode=PLINMode.MASTER, baudrate=19200)
        self.plin.set_id_filter(bytearray([0xFF] * 8))

        self.rgb_frame = self.add_master_frame("eval_0_rgb")

    def add_master_frame(self, name: str) -> Frame:
        return Frame(self.plin, self.db.get_frame(name))

    def run(self):
        self.plin.start_schedule(SCHEDULER_SLOT)

        color = [0, 0, 0]
        ch = 0
        while True:
            self.rgb_frame.update(
                {
                    "eval_0_rgb_r": color[0],
                    "eval_0_rgb_g": color[1],
                    "eval_0_rgb_b": color[2],
                }
            )

            color[ch] += 30
            if color[ch] > 255:
                color[ch] = 0
                ch = (ch + 1) % 3

            result = os.read(self.plin.fd, PLINMessage.buffer_length)
            frame = PLINMessage.from_buffer_copy(result)
            # frame = plin.read()
            if frame:
                print(f"{frame.id: 2x} ", end="")
                if frame.flags:
                    print(PLINFrameErrorFlag(frame.flags))
                else:
                    print(f"[{frame.len}] {bytearray(frame.data[:frame.len]).hex(' ')}")


Scheduler().run()
