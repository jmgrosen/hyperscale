import asyncio
from primitives import WaitAny

import lvgl as lv

from observable import Observable


GRAM_DIGIT_POSITIONS = [
    (0, 22),
    (75, 0),
    (155, 0),
    (235, 0),
    (315, 0),
    (45, 0),
    (395, 0),
]
OZ_DIGIT_POSITIONS = [
    (0, 22),
    (45, 0),
    (155, 0),
    (235, 0),
    (315, 0),
    (125, 0),
    (395, 0),
]
SIZE = [536, 240]
DIGIT_CHARS = "0123456789"

class WeightValueView:
    def __init__(self, weight: Observable, unit: Observable, parent=None, font=lv.font_montserrat_48, font_small=lv.font_montserrat_16, show_leading_zeros=True):
        self.root = lv.obj(parent)
        self.root.clear_flag(lv.obj.FLAG.SCROLLABLE)
        self.root.set_size(*SIZE)
        self.weight = weight
        self.unit = unit
        self.show_leading_zeros = show_leading_zeros

        self.dot = lv.label(self.root)
        self.dot.set_text(".")
        self.digits = [lv.label(self.root) for _ in range(5)]
        self.neg = lv.label(self.root)

        for i, d in enumerate(self.digits + [self.dot, self.neg]):
            d.set_style_text_font(font if i > 0 else font_small, 0)

    async def render(self):
        await asyncio.gather(self.render_weight(), self.update_positions())

    async def render_weight(self):
        update_event = WaitAny((self.weight.event, self.unit.event))
        while True:
            # TODO: support ounces
            if self.weight.value is not None:
                conversion = 3527.396195 if self.unit.value == 1 else 10000
                weight_int = round(self.weight.value * conversion)
                self.neg.set_text("-" if weight_int < 0 else "")
                weight_nat = abs(weight_int)
                for i, d in enumerate(self.digits):
                    if i < 2 or weight_nat > 0 or self.show_leading_zeros:
                        d.set_text(DIGIT_CHARS[weight_nat % 10])
                    else:
                        d.set_text("")
                    weight_nat = weight_nat // 10
            else:
                for d in self.digits + [self.neg]:
                    d.set_text("-")

            await update_event.wait()

    async def update_positions(self):
        while True:
            digit_positions = OZ_DIGIT_POSITIONS if self.unit.value == 1 else GRAM_DIGIT_POSITIONS
            for d, (pos_x, pos_y) in zip(self.digits + [self.dot, self.neg], digit_positions):
                d.align(lv.ALIGN.RIGHT_MID, -pos_x, pos_y)

            await self.unit.event.wait()
