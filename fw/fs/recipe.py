from observable import Observable


class Ingredient:
    def __init__(self, name, amount):
        self.name = name
        self.amount = amount

class Recipe:
    def __init__(self, name, ingredients):
        self.name = name
        self.ingredients = ingredients

class InProgressIngredient:
    def __init__(self, done, amount):
        self.done = done
        self.amount = amount

class InProgressRecipe:
    def __init__(self, recipe, progress=None):
        self.recipe = recipe
        self.progress = \
            progress if \
            progress is not None else \
            [Observable(InProgressIngredient(False, 0.0)) for _ in recipe.ingredients]

cookie_recipe = Recipe("cookies", [
    Ingredient("chocolate", 0.395),
    Ingredient("all purpose flour", 0.355),
    Ingredient("unsalted butter", 0.225),
    Ingredient("white sugar", 0.205),
    Ingredient("light brown sugar", 0.225),
    Ingredient("vanilla extract", 0.015),
    Ingredient("kosher salt", 0.008),
    Ingredient("baking soda", 0.001),
    Ingredient("baking powder", 0.001),
    Ingredient("nutmeg", 0.001),
    Ingredient("large egg", 0.050),
])
