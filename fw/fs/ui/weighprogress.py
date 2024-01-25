import asyncio
from primitives import WaitAny

import lvgl as lv

from observable import Observable
from .weight import WeightValueView


ingredients = [
    "chocolate",
    "all-purpose flour",
    "unsalted butter",
    "white sugar",
    "light brown sugar",
    "vanilla extract",
    "kosher salt",
    "baking soda",
    "baking powder",
    "nutmeg",
    "large egg",
]

SIZE = [536, 240]
BAR_HEIGHT = 128

def print_cb(s):
    def cb(e):
        print(s)
    return cb

class WeighingIngredientView:
    def __init__(self, weight: Observable, unit: Observable, ingredient, ingredient_status: Observable, parent=None, font_digits_large=lv.font_montserrat_48, font_digits_small=lv.font_montserrat_16, font_text=lv.font_montserrat_48):
        self.weight = weight
        self.ingredient = ingredient
        self.ingredient_status = ingredient_status
        self.should_update_status = False

        self.root = lv.obj(parent)
        self.root.set_style_pad_all(22, 0)
        self.root.set_size(*SIZE)
        self.root.clear_flag(lv.obj.FLAG.SCROLLABLE)

        self.group = lv.group_create()
        self.group.add_obj(self.root)

        self.root.set_layout(lv.LAYOUT_GRID.value)
        self.root.set_grid_dsc_array(
            [lv.grid_fr(1), lv.GRID_CONTENT, lv.GRID_TEMPLATE_LAST],
            [BAR_HEIGHT, lv.grid_fr(1), lv.GRID_TEMPLATE_LAST],
        )

        self.progress_obj = lv.obj(self.root)
        self.progress_obj.remove_style_all()
        self.progress_obj.set_grid_cell(lv.GRID_ALIGN.STRETCH, 0, 2, lv.GRID_ALIGN.STRETCH, 0, 1)

        bar_bg_style = lv.style_t()
        bar_bg_style.set_bg_color(lv.palette_main(lv.PALETTE.BLUE)) # TODO: use theme color
        bar_bg_style.set_bg_opa(lv.OPA._20)
        self.bar = lv.bar(self.progress_obj)
        self.bar.add_style(bar_bg_style, lv.PART.MAIN)
        self.bar.set_size(lv.pct(100), lv.pct(100))
        self.bar.set_range(0, 1000)
        self.bar.set_value(632, lv.ANIM.OFF)

        self.weight_view = WeightValueView(weight, unit, parent=self.progress_obj, font=font_digits_large, font_small=font_digits_small, show_leading_zeros=False)
        self.weight_view.root.set_size(lv.pct(100), lv.pct(100))
        self.weight_view.root.set_style_bg_opa(lv.OPA.TRANSP, 0)
        # can we set two?

        self.ingredient_name = lv.label(self.root)
        self.ingredient_name.set_text(ingredient.name)
        self.ingredient_name.set_style_text_font(font_text, 0)
        self.ingredient_name.set_grid_cell(lv.GRID_ALIGN.START, 0, 1, lv.GRID_ALIGN.END, 1, 1)

        self.ingredient_amount = lv.label(self.root)
        self.ingredient_amount.set_text(f"{round(ingredient.amount * 1000)} g")
        self.ingredient_amount.set_style_text_font(font_text, 0)
        self.ingredient_amount.set_grid_cell(lv.GRID_ALIGN.END, 1, 1, lv.GRID_ALIGN.END, 1, 1)
        
        
    async def render(self):
        await asyncio.gather(self.weight_view.render(), self.render_bar(), self.update_status())

    async def render_bar(self):
        update_event = WaitAny((self.weight.event, self.ingredient_status.event))
        while True:
            if self.visible:
                if self.weight.value is None:
                    self.bar.set_value(0, lv.ANIM.ON)
                else:
                    weight = self.weight.value if not self.ingredient_status.value.done else self.ingredient_status.value.amount
                    ratio = weight / self.ingredient.amount
                    self.bar.set_value(round(ratio * 1000), lv.ANIM.ON)

            await update_event.wait()

    async def update_status(self):
        while True:
            if self.should_update_status and self.weight is not None:
                self.ingredient_status.value.amount = self.weight.value
                self.ingredient_status.trigger()

            await self.weight.event.wait()

            
