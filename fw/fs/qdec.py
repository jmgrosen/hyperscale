"""Provides a Class to decode quadrature encoder output from two Pin's

Adapted from https://www.best-microcontroller-projects.com/rotary-encoder.html

Copyright (C) 2021 Mark Grosen <mark@grosen.org> (@mgsb)
SPDX-License-Identifier: MIT
"""

from machine import Pin, Timer


class QDec:
    """Decode quadrature (rotary) encoder output"""

    _VALID_ROT = [
        False, True, True, False,
        True, False, False, True,
        True, False, False, True,
        False, True, True, False
    ]

    def __init__(self, pin_a, pin_b, limit=None):
        self.pin_a = pin_a
        self.pin_b = pin_b
        self.count = 0

        self._prev = 0
        self._store = 0

        if isinstance(limit, int):
            def bound(val):
                if val < 0:
                    val = 0
                elif val > limit:
                    val = limit

                return val

            self._limit = bound
        elif callable(limit):
            self._limit = limit
        else:
            self._limit = lambda v: v

        qdec = self

        def _isr(_):
            cur = (qdec.pin_a.value() << 1) | qdec.pin_b.value()
            qdec._prev = ((qdec._prev << 2) | cur) & 0x0F
            if qdec._VALID_ROT[qdec._prev]:
                qdec._store = ((qdec._store & 0x0F) << 4) | qdec._prev
                if qdec._store == 0x2B:
                    inc = -1
                elif qdec._store == 0x17:
                    inc = 1
                else:
                    inc = 0

                qdec.count = qdec._limit(qdec.count + inc)

        # pin_a.irq(trigger=Pin.IRQ_FALLING|Pin.IRQ_RISING, handler=_isr)
        # pin_b.irq(trigger=Pin.IRQ_FALLING|Pin.IRQ_RISING, handler=_isr)
        self._timer = Timer(0, period=1, callback=_isr)

    def deinit(self):
        if self._timer:
            self._timer.deinit()
