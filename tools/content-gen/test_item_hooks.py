import unittest
from generate import registration

class ItemHookCatalog(unittest.TestCase):
    def test_factory_lambda_statement_keeps_nested_semicolons_and_name(self):
        source = '''GameRegistry.registerItem(radaway, radaway.getUnlocalizedName());
        ModItems.radaway = new ItemSimpleConsumable().setUseActionServer((stack, user) -> {
            use(stack);
            after(user);
        }).setUnlocalizedName("radaway").setTextureName(RefStrings.MODID + ":radaway");
        next = new Item().setUnlocalizedName("other");'''
        self.assertEqual(registration(source, "radaway", "item"), ("radaway", "radaway"))
