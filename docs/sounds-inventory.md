# Inventário de áudio Java → port

Gerado por `python3 tools/sound-gen/generate.py --source PATH`. Não editar esta tabela.

Fonte `4f5924e2b9e1e49cee61787674d2658acb3d559b`. 1165 chamadas ativas, incluindo loops; argumentos preservam expressões Java. `not_ported` significa fora dos sistemas com runtime mapeados neste lote. A presença de um ponto no port não atesta paridade de cada ramo: ver `docs/sons-hbm.md`.

| Java | Chamada | Evento/expressão | Volume | Pitch | Ponto do port |
| --- | --- | --- | --- | --- | --- |
| src/main/java/com/hbm/blocks/bomb/BlockChargeBase.java:120 | playSoundEffect | hbm:weapon.fstbmbStart | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/bomb/BlockChargeBase.java:175 | playSoundEffect | hbm:weapon.fstbmbStart | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/bomb/BlockChargeBase.java:188 | playSoundEffect | hbm:item.techBoop | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/bomb/BlockCrashedBomb.java:129 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/bomb/BlockTNTBase.java:82 | playSoundAtEntity | game.tnt.primed | 1.0F | 1.0F | crates/ntm-core/src/blast/visuals.rs |
| src/main/java/com/hbm/blocks/bomb/BombFloat.java:59 | playSoundEffect | hbm:weapon.sparkShoot | 5.0f | world.rand.nextFloat() * 0.2F + 0.9F | not_ported |
| src/main/java/com/hbm/blocks/bomb/Landmine.java:212 | playSoundEffect | hbm:weapon.mukeExplosion | 25.0F | 0.9F | not_ported |
| src/main/java/com/hbm/blocks/bomb/NukeBoy.java:125 | playSoundEffect | random.explode | 1.0f | world.rand.nextFloat() * 0.1F + 0.9F | crates/ntm-core/src/blast/visuals.rs |
| src/main/java/com/hbm/blocks/bomb/NukeCustom.java:156 | playSoundEffect | random.explode | 100000.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/bomb/NukeFleija.java:124 | playSoundEffect | random.explode | 1.0f | world.rand.nextFloat() * 0.1F + 0.9F | not_ported |
| src/main/java/com/hbm/blocks/bomb/NukeGadget.java:131 | playSoundEffect | random.explode | 1.0f | world.rand.nextFloat() * 0.1F + 0.9F | crates/ntm-core/src/blast/visuals.rs |
| src/main/java/com/hbm/blocks/bomb/NukeMan.java:124 | playSoundEffect | random.explode | 1.0f | world.rand.nextFloat() * 0.1F + 0.9F | crates/ntm-core/src/blast/visuals.rs |
| src/main/java/com/hbm/blocks/bomb/NukeMike.java:131 | playSoundEffect | random.explode | 1.0f | world.rand.nextFloat() * 0.1F + 0.9F | crates/ntm-core/src/blast/visuals.rs |
| src/main/java/com/hbm/blocks/bomb/NukeN2.java:120 | playSoundEffect | random.explode | 1.0f | world.rand.nextFloat() * 0.1F + 0.9F | crates/ntm-core/src/blast/visuals.rs |
| src/main/java/com/hbm/blocks/bomb/NukePrototype.java:135 | playSoundEffect | random.explode | 1.0f | world.rand.nextFloat() * 0.1F + 0.9F | not_ported |
| src/main/java/com/hbm/blocks/bomb/NukeSolinium.java:122 | playSoundEffect | random.explode | 1.0f | world.rand.nextFloat() * 0.1F + 0.9F | not_ported |
| src/main/java/com/hbm/blocks/bomb/NukeTsar.java:130 | playSoundEffect | random.explode | 1.0f | world.rand.nextFloat() * 0.1F + 0.9F | crates/ntm-core/src/blast/visuals.rs |
| src/main/java/com/hbm/blocks/fluid/GenericFluidBlock.java:100 | playSoundAtEntity | random.fizz | 0.2F | 1F | not_ported |
| src/main/java/com/hbm/blocks/fluid/VolcanicBlock.java:147 | playSound | liquid.lavapop | 0.2F + rand.nextFloat() * 0.2F | 0.9F + rand.nextFloat() * 0.15F | not_ported |
| src/main/java/com/hbm/blocks/fluid/VolcanicBlock.java:151 | playSound | liquid.lava | 0.2F + rand.nextFloat() * 0.2F | 0.9F + rand.nextFloat() * 0.15F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockAmmoCrate.java:49 | playSoundEffect | hbm:block.crateBreak | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockCanCrate.java:46 | playSoundEffect | hbm:block.crateBreak | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockCrate.java:45 | playSoundEffect | hbm:block.crateBreak | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockForgottenLock.java:95 | playSoundAtEntity | hbm:block.lockOpen | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockKeyhole.java:71 | playSoundAtEntity | hbm:block.lockOpen | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockModDoor.java:237 | playSoundEffect | hbm:block.openDoor | 1.0F | p_149727_1_.rand.nextFloat() * 0.1F + 0.9F | content/data/ntm/horizon/block/door_*.json |
| src/main/java/com/hbm/blocks/generic/BlockModDoor.java:261 | playSoundEffect | hbm:block.openDoor | 1.0F | p_150014_1_.rand.nextFloat() * 0.1F + 0.9F | content/data/ntm/horizon/block/door_*.json |
| src/main/java/com/hbm/blocks/generic/BlockOre.java:202 | playSoundEffect | random.fizz | 0.5F | 2.6F + (world.rand.nextFloat() - world.rand.nextFloat()) * 0.8F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockPlushie.java:106 | playSoundEffect | hbm:block.hunduns_magnificent_howl | 100F | 1F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockPlushie.java:108 | playSoundEffect | hbm:block.squeakyToy | 0.25F | 1F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockRedBrickKeyhole.java:93 | playSoundAtEntity | hbm:block.lockOpen | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/BlockSupplyCrate.java:85 | playSoundEffect | hbm:block.crateBreak | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/HEVBattery.java:78 | playSoundAtEntity | hbm:item.battery | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/Spikes.java:35 | playSoundEffect | hbm:entity.slicer | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/TrappedBrick.java:72 | playSoundEffect | hbm:entity.slicer | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/generic/TrappedBrick.java:121 | playSoundEffect | random.click | 0.3F | 0.6F | not_ported |
| src/main/java/com/hbm/blocks/machine/BlockVendingMachine.java:70 | playSoundEffect | hbm:weapon.reload.boltOpen | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/blocks/machine/FoundryCastingBase.java:105 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/machine/GeigerCounter.java:127 | playSoundAtEntity | hbm:item.techBoop | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/machine/MachineChungus.java:68 | playSoundEffect | hbm:block.chungusLever | 1.5F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/machine/MachineConveyorPress.java:81 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/machine/MachineCyclotron.java:54 | playSoundEffect | hbm:item.upgradePlug | 1.5F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/machine/MachineFan.java:207 | playSoundEffect | random.click | 0.5F | 0.5F | not_ported |
| src/main/java/com/hbm/blocks/machine/MachineFan.java:223 | playSoundEffect | random.click | 0.5F | 0.5F | not_ported |
| src/main/java/com/hbm/blocks/machine/MachineIndustrialTurbine.java:59 | playSoundEffect | hbm:block.chungusLever | 1.5F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/machine/MachineSatLink.java:79 | playSoundAtEntity | hbm:item.techBleep | 1F | 1F | not_ported |
| src/main/java/com/hbm/blocks/machine/MachineSawmill.java:101 | playSoundEffect | hbm:item.upgradePlug | 1.5F | 0.75F | not_ported |
| src/main/java/com/hbm/blocks/machine/MachineStirling.java:98 | playSoundEffect | hbm:item.upgradePlug | 1.5F | 0.75F | crates/ntm-core/src/solar_stirling.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/blocks/machine/MachineStrandCaster.java:148 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/machine/PistonInserter.java:246 | playSoundEffect | hbm:block.pressOperate | 1.0F | 1.5F | not_ported |
| src/main/java/com/hbm/blocks/machine/Radiobox.java:64 | playSoundEffect | hbm:item.upgradePlug | 1.5F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/machine/Radiobox.java:73 | playSoundEffect | hbm:block.reactorStart | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/machine/Radiobox.java:76 | playSoundEffect | hbm:block.reactorStart | 1.0F | 0.85F | not_ported |
| src/main/java/com/hbm/blocks/machine/ZirnoxDestroyed.java:94 | playSoundEffect | fire.fire | 1.0F + world.rand.nextFloat() | world.rand.nextFloat() * 0.7F + 0.3F | crates/ntm-core/src/server/zirnox_server.rs |
| src/main/java/com/hbm/blocks/machine/pile/BlockGraphiteDrilled.java:54 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | crates/ntm-core/src/server/pile_server.rs |
| src/main/java/com/hbm/blocks/machine/pile/BlockGraphiteDrilled.java:73 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 0.85F | crates/ntm-core/src/server/pile_server.rs |
| src/main/java/com/hbm/blocks/machine/pile/BlockGraphiteDrilledBase.java:224 | playSoundEffect | hbm:item.upgradePlug | 1.25F | 1.0F | crates/ntm-core/src/server/pile_server.rs |
| src/main/java/com/hbm/blocks/machine/pile/BlockGraphiteNeutronDetector.java:66 | playSoundEffect | hbm:item.techBleep | 0.02F | 1.0F | crates/ntm-core/src/server/pile_server.rs |
| src/main/java/com/hbm/blocks/machine/pile/BlockGraphiteRod.java:65 | playSoundEffect | random.click | 0.3F | pureMeta == (oldMeta & 11) ? 0.75F : 0.65F | crates/ntm-core/src/server/pile_server.rs |
| src/main/java/com/hbm/blocks/machine/pile/BlockPileDevice.java:102 | playSoundEffect | hbm:item.upgradePlug | 1F | 1F | crates/ntm-core/src/server/pile_server.rs |
| src/main/java/com/hbm/blocks/machine/rbmk/RBMKDebrisBurning.java:37 | playSoundEffect | fire.fire | 1.0F + rand.nextFloat() | rand.nextFloat() * 0.7F + 0.3F | not_ported |
| src/main/java/com/hbm/blocks/machine/rbmk/RBMKDebrisRadiating.java:48 | playSoundEffect | fire.fire | 1.0F + rand.nextFloat() | rand.nextFloat() * 0.7F + 0.3F | not_ported |
| src/main/java/com/hbm/blocks/machine/rbmk/RBMKRod.java:86 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/network/CableDetector.java:52 | playSoundEffect | hbm:block.reactorStart | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/network/CableDetector.java:58 | playSoundEffect | hbm:block.reactorStart | 1.0F | 0.85F | not_ported |
| src/main/java/com/hbm/blocks/network/CableSwitch.java:52 | playSoundEffect | hbm:block.reactorStart | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/network/CableSwitch.java:55 | playSoundEffect | hbm:block.reactorStart | 1.0F | 0.85F | not_ported |
| src/main/java/com/hbm/blocks/network/FluidDuctBase.java:71 | playSoundAtEntity | random.orb | 0.25F | 0.75F | not_ported |
| src/main/java/com/hbm/blocks/network/FluidDuctBase.java:94 | playSoundAtEntity | random.orb | 0.25F | 0.75F | not_ported |
| src/main/java/com/hbm/blocks/network/FluidSwitch.java:58 | playSoundEffect | hbm:block.reactorStart | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/network/FluidSwitch.java:64 | playSoundEffect | hbm:block.reactorStart | 1.0F | 0.85F | not_ported |
| src/main/java/com/hbm/blocks/network/FluidValve.java:60 | playSoundEffect | hbm:block.reactorStart | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/blocks/network/FluidValve.java:63 | playSoundEffect | hbm:block.reactorStart | 1.0F | 0.85F | not_ported |
| src/main/java/com/hbm/entity/cart/EntityMinecartDestroyer.java:122 | playSoundEffect | mob.zombie.woodbreak | 0.5F | 0.5F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/effect/EntityMist.java:339 | playSoundEffect | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/effect/EntityMist.java:340 | playSound | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/effect/EntityNukeTorex.java:108 | playSoundClient | hbm:weapon.nuclearExplosion | 10_000F | 1F | crates/ntm-core/src/blast/visuals.rs |
| src/main/java/com/hbm/entity/effect/EntitySpear.java:108 | playSoundEffect | hbm:weapon.dFlash | 25000.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/grenade/EntityGrenadeBouncyBase.java:160 | playSoundAtEntity | hbm:weapon.gBounce | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/grenade/EntityGrenadeUniversal.java:137 | playSoundEffect | hbm:weapon.grenadeBounce | 1F | 1F | not_ported |
| src/main/java/com/hbm/entity/item/EntityFireworks.java:43 | playSoundEffect | fireworks.blast | 20 | 1F + this.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/item/EntityMinecartTest.java:157 | playSoundAtEntity | game.tnt.primed | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/item/EntityRequestDrone.java:105 | playSoundEffect | hbm:item.unpack | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/entity/item/EntityRequestDrone.java:146 | playSoundEffect | hbm:item.unpack | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/entity/item/EntityRequestDrone.java:173 | playSoundEffect | hbm:block.storageClose | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/item/EntityRequestDrone.java:183 | playSoundEffect | hbm:block.storageClose | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityBomber.java:47 | getLoopedSound | bomberType <= 4 ? "hbm:entity.bomberSmallLoop" : "hbm:entity.bomberLoop" | 2F | 1F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityBomber.java:63 | playSoundEffect | random.fizz | 5.0F | 2.6F + (rand.nextFloat() - rand.nextFloat()) * 0.8F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityBomber.java:69 | playSoundEffect | hbm:weapon.missileTakeOff | 10.0F | 0.9F + rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityBomber.java:77 | playSoundEffect | random.fizz | 5.0F | 2.6F + (rand.nextFloat() - rand.nextFloat()) * 0.8F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityBomber.java:81 | playSoundEffect | hbm:entity.bombWhistle | 10.0F | 0.9F + rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityC130.java:32 | getLoopedSound | hbm:entity.bomberLoop | 2F | 1F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityDeathBlast.java:42 | playSoundEffect | hbm:weapon.mukeExplosion | 25.0F | 0.9F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityNukeExplosionMK3.java:182 | playSoundEffect | ambient.weather.thunder | 10000.0F | 0.8F + this.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityNukeExplosionMK3.java:247 | playSoundEffect | hbm:entity.ufoBlast | 15.0F | 0.7F + world.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityPlaneBase.java:66 | playSoundEffect | hbm:entity.planeShotDown | 25.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityPlaneBase.java:130 | playSoundEffect | hbm:entity.planeCrash | 25.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityTomBlast.java:84 | playSoundEffect | random.explode | 10000.0F | 0.8F + this.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/logic/EntityTomBlast.java:87 | playSoundEffect | ambient.weather.thunder | 10000.0F | 0.8F + this.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/missile/EntityBobmazon.java:46 | playSoundEffect | hbm:entity.oldExplosion | 10.0F | 0.5F + this.rand.nextFloat() * 0.1F | not_ported |
| src/main/java/com/hbm/entity/missile/EntityMissileShuttle.java:43 | playSoundEffect | hbm:weapon.robin_explosion | 4.0F | (1.0F + (this.worldObj.rand.nextFloat() - this.worldObj.rand.nextFloat()) * 0.2F) * 0.7F | not_ported |
| src/main/java/com/hbm/entity/missile/EntityRocketSoyuz.java:47 | playSoundEffect | hbm:alarm.soyuzed | 100 | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityBurrowingSwingingBase.java:61 | playSoundAtEntity | hbm:block.debris | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityCreeperNuclear.java:120 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityCyberCrab.java:160 | playSound | hbm:weapon.sawShoot | 1.0F | 2.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityHunterChopper.java:116 | playSoundEffect | hbm:misc.nullChopper | 10.0F | 0.5F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityHunterChopper.java:180 | playSoundAtEntity | hbm:weapon.osiprShoot | 10.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityHunterChopper.java:204 | playSoundAtEntity | hbm:entity.chopperCharge | 5.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityHunterChopper.java:209 | playSoundAtEntity | hbm:entity.chopperDrop | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityHunterChopper.java:270 | playSoundEffect | hbm:misc.nullCrashing | 10.0F | 0.5F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityHunterChopper.java:394 | playSoundAtEntity | hbm:entity.chopperDamage | 10.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityPigeon.java:97 | playSound | mob.chicken.step | 0.15F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityTaintCrab.java:82 | playSound | hbm:weapon.sawShoot | 1.0F | 0.5F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityUFO.java:183 | playSoundAtEntity | hbm:entity.ufoBeam | 10.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityUFO.java:298 | playSoundAtEntity | hbm:entity.chopperDamage | 10.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityUFO.java:334 | playSound | hbm:weapon.ballsLaser | 5.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityUFO.java:348 | playSound | hbm:turret.richard_fire | 5.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityUndeadSoldier.java:106 | playSound | mob.zombie.step | 0.15F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/EntityUndeadSoldier.java:107 | playSound | mob.skeleton.step | 0.15F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/ai/EntityAIBreaking.java:117 | playSoundAtEntity | block.stepSound.getStepResourcePath() | block.stepSound.getVolume() + 1F | block.stepSound.getPitch() | not_ported |
| src/main/java/com/hbm/entity/mob/ai/EntityAIEatBread.java:67 | playSound | random.eat | 0.5F + 0.5F * this.pigeon.getRNG().nextInt(2) | (this.pigeon.getRNG().nextFloat() - this.pigeon.getRNG().nextFloat()) * 0.2F + 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/ai/EntityAIMaskmanLasergun.java:59 | playSound | hbm:weapon.teslaShoot | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/ai/EntityAIMaskmanLasergun.java:70 | playSound | hbm:weapon.hkShoot | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/ai/EntityAIMaskmanMinigun.java:56 | playSound | hbm:weapon.calShoot | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/botprime/EntityBOTPrimeBase.java:87 | playSound | hbm:weapon.ballsLaser | 5.0F | 0.75F | not_ported |
| src/main/java/com/hbm/entity/mob/botprime/EntityBOTPrimeBase.java:92 | playSound | hbm:weapon.ballsLaser | 5.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/glyphid/EntityGlyphid.java:348 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/mob/glyphid/EntityGlyphid.java:397 | playSoundAtEntity | mob.zombie.woodbreak | 1.0F | 1.25F | not_ported |
| src/main/java/com/hbm/entity/mob/glyphid/EntityGlyphidNuclear.java:162 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/glyphid/EntityGlyphidNuclear.java:176 | playSoundEffect | hbm:weapon.fstbmbPing | 5.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/siege/EntitySiegeCraft.java:61 | playSoundAtEntity | random.break | 5F | 1.0F + rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/entity/mob/siege/EntitySiegeCraft.java:83 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/mob/siege/EntitySiegeCraft.java:189 | playSoundAtEntity | hbm:weapon.stingerLockOn | 2F | 0.75F | not_ported |
| src/main/java/com/hbm/entity/mob/siege/EntitySiegeCraft.java:209 | playSoundEffect | hbm:entity.ufoBlast | 5.0F | 0.9F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/mob/siege/EntitySiegeCraft.java:273 | playSound | hbm:weapon.ballsLaser | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityArtilleryShell.java:142 | playSoundEffect | hbm:turret.mortarWhistle | 15.0F | 0.9F + rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityB92Beam.java:445 | playSoundEffect | random.explode | 100.0f | this.worldObj.rand.nextFloat() * 0.1F + 0.9F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityBombletZeta.java:73 | playSoundEffect | random.fizz | 5.0F | 2.6F + (rand.nextFloat() - rand.nextFloat()) * 0.8F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityBombletZeta.java:88 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityBoxcar.java:55 | playSoundEffect | hbm:weapon.trainImpact | 100.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityBuilding.java:54 | playSoundEffect | hbm:entity.oldExplosion | 10000.0F | 0.5F + this.rand.nextFloat() * 0.1F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityBullet.java:336 | playSound | dig.glass | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityBulletBaseNT.java:260 | playSoundAtEntity | hbm:weapon.ricochet | 0.25F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityBulletBaseNT.java:262 | playSoundAtEntity | hbm:weapon.gBounce | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityBulletBaseNT.java:333 | playSoundEffect | mob.zombie.woodbreak | 1.0F | 0.95F + rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityChemical.java:374 | playSoundEffect | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityChemical.java:375 | playSound | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityChopperMine.java:121 | playSoundAtEntity | hbm:misc.nullMine | 10.0F | 1F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityCog.java:91 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityDuchessGambit.java:56 | playSoundEffect | hbm:alarm.gambit | 10000.0F | 1F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityMeteor.java:143 | playSoundEffect | hbm:entity.oldExplosion | 10000.0F | 0.5F + this.rand.nextFloat() * 0.1F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityMeteor.java:157 | getLoopedSound | hbm:entity.meteoriteFallingLoop | 1F | 0.9F + this.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityRubble.java:43 | playSoundAtEntity | hbm:block.debris | 1.5F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntitySawblade.java:85 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityShrapnel.java:96 | playSoundEffect | random.fizz | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/entity/projectile/EntityTom.java:28 | playSoundEffect | hbm:alarm.chime | 10000 | 1.0F | not_ported |
| src/main/java/com/hbm/explosion/ExplosionNT.java:187 | playSoundEffect | random.explode | 4.0F | (1.0F + (this.worldObj.rand.nextFloat() - this.worldObj.rand.nextFloat()) * 0.2F) * 0.7F | not_ported |
| src/main/java/com/hbm/explosion/ExplosionNukeSmall.java:32 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | crates/ntm-core/src/blast/visuals.rs |
| src/main/java/com/hbm/explosion/vanillant/standard/ExplosionEffectAmat.java:18 | playSoundEffect | random.explode | 4.0F | (1.4F + (world.rand.nextFloat() - world.rand.nextFloat()) * 0.2F) * 0.7F | not_ported |
| src/main/java/com/hbm/explosion/vanillant/standard/ExplosionEffectAmat.java:20 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/explosion/vanillant/standard/ExplosionEffectStandard.java:23 | playSoundEffect | random.explode | 4.0F | (1.0F + (world.rand.nextFloat() - world.rand.nextFloat()) * 0.2F) * 0.7F | not_ported |
| src/main/java/com/hbm/explosion/vanillant/standard/ExplosionEffectTiny.java:18 | playSoundEffect | hbm:weapon.explosionTiny | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/extprop/HbmPlayerProps.java:174 | playSoundAtEntity | sound | volume | pitch | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:299 | playSoundEffect | hbm:player.vomit | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:314 | playSoundEffect | hbm:player.vomit | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:470 | playSoundEffect | hbm:player.vomit | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:535 | playSoundEffect | hbm:player.cough | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:645 | playSoundEffect | random.fizz | 1F | 1.5F + rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:652 | playSoundEffect | random.fizz | 1F | 1.5F + rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:659 | playSoundEffect | random.fizz | 1F | 1.5F + rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:667 | playSoundEffect | random.fizz | 1F | 1.5F + rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:744 | playSound | hbm:weapon.rocketFlame | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/handler/EntityEffectHandler.java:758 | playSound | hbm:item.techBoop | 1.0F | (1.0F + ((1F/12F)*(stamina/perDash))) | not_ported |
| src/main/java/com/hbm/handler/ability/IWeaponAbility.java:264 | playSoundEffect | hbm:weapon.chainsaw | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/handler/guncfg/GunNPCFactory.java:293 | playSoundEffect | hbm:entity.ufoBlast | 5.0F | 0.9F + bulletnt.worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/handler/guncfg/GunNPCFactory.java:294 | playSoundEffect | fireworks.blast | 5.0F | 0.5F | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIAnvil.java:183 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIAnvil.java:191 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIAnvil.java:203 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIAnvil.java:210 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIAnvil.java:238 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIBarrel.java:50 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIBookLore.java:173 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIBookLore.java:178 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICombustionEngine.java:88 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICombustionEngine.java:95 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICompressor.java:50 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICoreEmitter.java:67 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICoreEmitter.java:73 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICoreStabilizer.java:64 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICounterTorch.java:90 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICounterTorch.java:97 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICraneBoxer.java:37 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICraneGrabber.java:57 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICraneInserter.java:48 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUICraneRouter.java:46 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIDiode.java:90 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIDroneCrate.java:50 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIElectrolyserFluid.java:46 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIElectrolyserMetal.java:62 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIFEL.java:45 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIForceField.java:51 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIFunnel.java:43 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUILaunchPadRusted.java:53 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineArcFurnaceLarge.java:55 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineBattery.java:73 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineBattery.java:78 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineBattery.java:83 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineDiesel.java:56 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineExcavator.java:60 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:block.leverLarge"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineFluidTank.java:50 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineGasFlare.java:48 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineGasFlare.java:54 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineLaunchTable.java:59 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineLaunchTable.java:65 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineLaunchTable.java:71 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineMissileAssembly.java:43 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineRadarNT.java:63 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineRadarNTSlots.java:39 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineSolderingStation.java:53 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineTurbineGas.java:58 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineTurbineGas.java:71 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineTurbineGas.java:83 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineWoodBurner.java:68 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMachineWoodBurner.java:75 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMassStorage.java:52 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMassStorage.java:60 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMiningLaser.java:56 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIMixer.java:75 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIOilburner.java:74 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIOilburner.java:81 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIPADipole.java:76 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIPADipole.java:83 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIPADipole.java:90 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIPASource.java:58 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIPWR.java:188 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIPump.java:100 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIPump.java:107 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKAutoloader.java:41 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKAutoloader.java:48 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKBoiler.java:47 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:140 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.75F + (this.selection[index] ? 0.25F : 0.0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:148 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:162 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.5F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:180 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:214 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.8F + k * 0.1F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:224 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:block.rbmk_az5_cover"), 0.5F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:226 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:block.shutdown"), 1) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:238 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:265 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:279 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKConsole.java:295 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.75F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKControl.java:52 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKControl.java:61 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKControlAuto.java:102 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRBMKControlAuto.java:133 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRTTYDesignator.java:81 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRadioRec.java:98 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIRadioRec.java:105 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIReactorControl.java:89 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIReactorControl.java:119 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIReactorResearch.java:114 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:block.rbmk_az5_cover"), 0.5F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIReactorZirnox.java:71 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:block.rbmk_az5_cover"), 0.5F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIReactorZirnox.java:78 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:block.rbmk_az5_cover"), 0.5F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUISILEX.java:59 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenBobble.java:30 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation(NTMSounds.BLOCK_FALLOUT_3_POPUP), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenBobmazon.java:189 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenCalculator.java:202 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenDesignator.java:189 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenFluid.java:103 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenFluid.java:109 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenFluid.java:115 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenGuide.java:233 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenGuide.java:242 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenGuide.java:247 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenHolotape.java:26 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:block.bobble"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenPager.java:81 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKDisplay.java:102 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.active[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKDisplay.java:108 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.polling[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKDisplay.java:114 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.shorten_number[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKDisplay.java:120 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.leading_zeroes[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKDisplay.java:126 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKGauge.java:110 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.active[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKGauge.java:116 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.polling[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKGauge.java:122 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKGraph.java:104 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.active[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKGraph.java:110 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.polling[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKGraph.java:116 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKIndicator.java:112 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.active[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKIndicator.java:118 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.polling[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKIndicator.java:124 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKKeyPad.java:117 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.active[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKKeyPad.java:123 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.polling[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKKeyPad.java:129 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKLever.java:107 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.active[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKLever.java:113 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F + (this.polling[i] ? 0.25F : 0F)) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRBMKLever.java:119 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioAUTOCAL.java:149 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTelex.java:234 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTelex.java:251 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorch.java:136 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorch.java:143 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorch.java:150 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorchController.java:120 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorchController.java:127 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorchLogic.java:146 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorchLogic.java:153 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorchLogic.java:160 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorchLogic.java:170 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorchReader.java:129 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRadioTorchReader.java:136 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenRecipeSelector.java:348 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenSatCoord.java:76 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenSnowglobe.java:27 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation(NTMSounds.BLOCK_FALLOUT_3_POPUP), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenToolAbility.java:298 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 0.5F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenToolAbility.java:316 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:item.techBoop"), 2F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIScreenToolAbility.java:401 | playSoundAtEntity | random.orb | 0.25F | config.getActivePreset().isNone() ? 0.75F : 1.25F | not_ported |
| src/main/java/com/hbm/inventory/gui/GUISoyuzLauncher.java:54 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUISoyuzLauncher.java:60 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUISoyuzLauncher.java:66 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretArty.java:68 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretBase.java:117 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretBase.java:124 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretBase.java:131 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretBase.java:138 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretBase.java:145 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretBase.java:159 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretBase.java:167 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretBase.java:174 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretBase.java:189 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUITurretHIMARS.java:37 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GUIWatz.java:66 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GuiInfoContainer.java:340 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/inventory/gui/GuiInfoContainer.java:345 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/items/ItemInventory.java:128 | playSoundEffect | hbm:block.crateOpen | 1.0F | 0.8F | not_ported |
| src/main/java/com/hbm/items/ItemInventory.java:129 | playSoundEffect | hbm:block.crateClose | 1.0F | 0.8F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorBJJetpack.java:72 | playSoundEffect | hbm:weapon.immolatorShoot | 0.125F | 1.5F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorDNT.java:107 | playSoundEffect | hbm:weapon.immolatorShoot | 0.125F | 1.5F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorDNT.java:127 | playSoundEffect | hbm:weapon.immolatorShoot | 0.125F | 1.5F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorFSB.java:301 | playSound | sound | 0.25F | 1.0F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorFSB.java:316 | playSound | chestplate.jump | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorFSB.java:350 | playSound | chestplate.fall | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorFSB.java:381 | playSoundAtEntity | "hbm:item.geiger" + r | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorHat.java:48 | playSoundAtEntity | random.break | 5F | 1.0F + event.entityLiving.getRNG().nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorNCRPAMelee.java:58 | playSoundAtEntity | hbm:weapon.fire.stab | 1F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorNCRPAMelee.java:62 | playSoundEffect | b.stepSound.getStepResourcePath() | 2F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorNCRPARanged.java:42 | playSoundAtEntity | hbm:weapon.rpgShoot | 0.5F | 0.9F + player.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorNCRPARanged.java:47 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorNo9.java:93 | playSoundAtEntity | fire.ignite | 1F | 1.5F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorNo9.java:94 | playSoundAtEntity | random.fizz | 0.5F | 2F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorRPAMelee.java:65 | playSoundAtEntity | hbm:weapon.fire.smack | 1F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/armor/ArmorRPAMelee.java:69 | playSoundEffect | b.stepSound.getStepResourcePath() | 2F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/armor/ItemModAuto.java:42 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/armor/ItemModDefuser.java:68 | playSoundEffect | hbm:item.pinBreak | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/armor/ItemModKnife.java:53 | playSoundAtEntity | hbm:entity.slicer | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/armor/ItemModSensor.java:70 | playSoundAtEntity | hbm:weapon.follyAquired | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/items/armor/ItemModSensor.java:72 | playSoundAtEntity | hbm:item.techBoop | 2F | 1.5F | not_ported |
| src/main/java/com/hbm/items/armor/JetpackBooster.java:65 | playSoundEffect | hbm:weapon.flamethrowerShoot | 0.25F | 1.0F | not_ported |
| src/main/java/com/hbm/items/armor/JetpackBreak.java:59 | playSoundEffect | hbm:weapon.flamethrowerShoot | 0.25F | 1.5F | not_ported |
| src/main/java/com/hbm/items/armor/JetpackBreak.java:75 | playSoundEffect | hbm:weapon.flamethrowerShoot | 0.25F | 1.5F | not_ported |
| src/main/java/com/hbm/items/armor/JetpackRegular.java:52 | playSoundEffect | hbm:weapon.flamethrowerShoot | 0.25F | 1.5F | not_ported |
| src/main/java/com/hbm/items/armor/JetpackVectorized.java:64 | playSoundEffect | hbm:weapon.flamethrowerShoot | 0.25F | 1.5F | not_ported |
| src/main/java/com/hbm/items/food/ItemBDCL.java:39 | playSound | hbm:player.gulp | 1F | 1F | native/ntm/mixins/ntm/item_hooks.rs, crates/ntm-core/src/consumables.rs |
| src/main/java/com/hbm/items/food/ItemBDCL.java:46 | playSound | hbm:player.groan | 1F | 1F | native/ntm/mixins/ntm/item_hooks.rs, crates/ntm-core/src/consumables.rs |
| src/main/java/com/hbm/items/food/ItemConserve.java:35 | playSoundAtEntity | random.burp | 0.5F | world.rand.nextFloat() * 0.1F + 0.9F | not_ported |
| src/main/java/com/hbm/items/food/ItemLemon.java:104 | playSoundEffect | hbm:entity.vomit | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs, crates/ntm-core/src/consumables.rs |
| src/main/java/com/hbm/items/machine/ItemFluidDuct.java:128 | playSoundEffect | ModBlocks.fluid_duct_neo.stepSound.func_150496_b() | (ModBlocks.fluid_duct_neo.stepSound.getVolume() + 1.0F) / 2.0F | ModBlocks.fluid_duct_neo.stepSound.getPitch() * 0.8F | not_ported |
| src/main/java/com/hbm/items/machine/ItemFluidIDMulti.java:55 | playSoundAtEntity | random.orb | 0.25F | 1.25F | not_ported |
| src/main/java/com/hbm/items/machine/ItemMuffler.java:22 | playSoundAtEntity | hbm:item.upgradePlug | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/machine/ItemRBMKLid.java:52 | playSoundEffect | Blocks.glass.stepSound.func_150496_b() | (Blocks.glass.stepSound.getVolume() + 1.0F) / 2.0F | Blocks.glass.stepSound.getPitch() * 0.8F | not_ported |
| src/main/java/com/hbm/items/machine/ItemRBMKLid.java:54 | playSoundEffect | ModBlocks.concrete_smooth.stepSound.func_150496_b() | (ModBlocks.concrete_smooth.stepSound.getVolume() + 1.0F) / 2.0F | ModBlocks.concrete_smooth.stepSound.getPitch() * 0.8F | not_ported |
| src/main/java/com/hbm/items/machine/ItemRTGPellet.java:132 | playSoundAtEntity | random.orb | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/machine/ItemReactorSensor.java:40 | playSoundAtEntity | hbm:item.techBoop | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/machine/ItemTurretBiometry.java:31 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/special/ItemCigarette.java:65 | playSoundEffect | hbm:player.cough | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs, crates/ntm-core/src/consumables.rs |
| src/main/java/com/hbm/items/special/ItemDrop.java:98 | playSoundEffect | random.explode | 100.0F | entityItem.worldObj.rand.nextFloat() * 0.1F + 0.9F | not_ported |
| src/main/java/com/hbm/items/special/ItemDrop.java:254 | playSoundAtEntity | hbm:item.techBoop | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/special/ItemKitNBT.java:49 | playSoundAtEntity | hbm:item.unpack | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/special/ItemPotatos.java:33 | playSoundAtEntity | hbm:potatos.random | 1.0F | pitch | not_ported |
| src/main/java/com/hbm/items/special/ItemSimpleConsumable.java:63 | playSoundAtEntity | sound | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemStarterKit.java:395 | playSoundAtEntity | hbm:item.unpack | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:42 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:61 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:81 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:101 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:140 | playSoundAtEntity | hbm:item.radaway | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:158 | playSoundAtEntity | hbm:item.radaway | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:167 | playSoundAtEntity | hbm:item.radaway | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:177 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:196 | playSoundAtEntity | hbm:item.gasmaskScrew | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:223 | playSoundAtEntity | hbm:item.jetpackTank | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:237 | playSoundAtEntity | hbm:item.vice | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:254 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:271 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:289 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:307 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:324 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemSyringe.java:341 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/special/ItemTeleLink.java:35 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/special/ItemTeleLink.java:44 | playSoundAtEntity | hbm:item.techBoop | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/special/ItemTeleLink.java:62 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/special/ItemUnstable.java:52 | playSoundAtEntity | hbm:entity.oldExplosion | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemAmmoContainer.java:81 | playSoundAtEntity | hbm:item.unpack | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemAnchorRemote.java:55 | playSoundAtEntity | random.orb | 0.25F | 0.75F | not_ported |
| src/main/java/com/hbm/items/tool/ItemAnchorRemote.java:60 | playSoundAtEntity | random.orb | 0.25F | 0.75F | not_ported |
| src/main/java/com/hbm/items/tool/ItemAnchorRemote.java:77 | playSoundEffect | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemAnchorRemote.java:89 | playSoundAtEntity | random.orb | 0.25F | 0.75F | not_ported |
| src/main/java/com/hbm/items/tool/ItemBalefireMatch.java:34 | playSoundEffect | fire.ignite | 1.0F | itemRand.nextFloat() * 0.4F + 0.8F | not_ported |
| src/main/java/com/hbm/items/tool/ItemBoltgun.java:60 | playSoundAtEntity | hbm:item.boltgun | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemBoltgun.java:97 | playSoundAtEntity | hbm:item.boltgun | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemBombCaller.java:67 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDesignatorArtyRange.java:60 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDesignatorArtyRange.java:85 | playSoundAtEntity | hbm:item.techBoop | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDesingator.java:45 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDesingatorRange.java:51 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDetonator.java:52 | playSoundAtEntity | hbm:item.techBoop | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDetonator.java:76 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDigammaDiagnostic.java:17 | playSoundAtEntity | hbm:item.techBoop | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDiscord.java:33 | playSoundEffect | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDiscord.java:37 | playSoundEffect | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemDosimeter.java:43 | playSoundAtEntity | NTMSounds.GEIGER_PREFIX + r | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/tool/ItemDosimeter.java:46 | playSoundAtEntity | NTMSounds.GEIGER_PREFIX + 1 | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/tool/ItemDosimeter.java:55 | playSoundAtEntity | hbm:item.techBoop | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/tool/ItemFilter.java:69 | playSoundAtEntity | hbm:item.gasmaskScrew | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/tool/ItemFusionCore.java:47 | playSoundAtEntity | hbm:item.battery | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemGeigerCounter.java:48 | playSoundAtEntity | NTMSounds.GEIGER_PREFIX + r | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/tool/ItemGeigerCounter.java:50 | playSoundAtEntity | NTMSounds.GEIGER_PREFIX + 1 | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/tool/ItemGeigerCounter.java:78 | playSoundAtEntity | hbm:item.techBoop | 1.0F | 1.0F | native/ntm/mixins/ntm/item_hooks.rs |
| src/main/java/com/hbm/items/tool/ItemLaserDetonator.java:50 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemLaserDetonator.java:54 | playSoundAtEntity | hbm:item.techBoop | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemLock.java:36 | playSoundAtEntity | hbm:block.lockHang | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemMatch.java:29 | playSoundEffect | fire.ignite | 1.0F | itemRand.nextFloat() * 0.4F + 0.8F | not_ported |
| src/main/java/com/hbm/items/tool/ItemMeteorRemote.java:39 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemModBucket.java:37 | playSoundEffect | random.fizz | 0.5F | 2.6F + (world.rand.nextFloat() - world.rand.nextFloat()) * 0.8F | not_ported |
| src/main/java/com/hbm/items/tool/ItemMultiDetonator.java:59 | playSoundAtEntity | hbm:item.techBoop | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemMultiDetonator.java:106 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemMultiDetonator.java:121 | playSoundAtEntity | hbm:item.techBoop | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemOilDetector.java:93 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemRadarLinker.java:29 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemRepairKit.java:47 | playSoundAtEntity | hbm:item.spray | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemRepairKit.java:48 | playSoundAtEntity | hbm:item.repair | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/ItemSwordAbility.java:57 | playSoundAtEntity | hbm:weapon.whack | 3.0F | 1.F | not_ported |
| src/main/java/com/hbm/items/tool/ItemToolAbility.java:603 | playSoundAtEntity | random.orb | 0.25F | config.getActivePreset().isNone() ? 0.75F : 1.25F | not_ported |
| src/main/java/com/hbm/items/tool/ItemToolingWeapon.java:39 | playSoundAtEntity | random.anvil_land | 3.0F | 0.75F | not_ported |
| src/main/java/com/hbm/items/tool/ItemWrench.java:117 | playSoundAtEntity | random.anvil_land | 3.0F | 0.75F | not_ported |
| src/main/java/com/hbm/items/tool/RedstoneSword.java:67 | playSoundEffect | random.break | 1.0F | itemRand.nextFloat() * 0.4F + 0.8F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:65 | playSoundAtEntity | hbm:weapon.bonk | 3.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:80 | playSoundAtEntity | random.anvil_land | 3.0F | 1.F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:100 | playSoundAtEntity | hbm:weapon.bang | 3.0F | 1.F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:106 | playSoundAtEntity | hbm:weapon.slice | 3.0F | 1.F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:110 | playSoundAtEntity | hbm:weapon.whack | 3.0F | 1.F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:114 | playSoundAtEntity | hbm:weapon.whack | 3.0F | 1.F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:121 | playSoundAtEntity | hbm:weapon.whack | 3.0F | 1.F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:135 | playSoundAtEntity | random.anvil_land | 3.0F | 0.75F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:144 | playSoundAtEntity | hbm:weapon.bang | 3.0F | 0.75F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:158 | playSoundAtEntity | hbm:weapon.stop | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:184 | playSoundAtEntity | hbm:weapon.bang | 3.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/tool/WeaponSpecial.java:197 | playSoundEffect | hbm:weapon.kapeng | 3.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/GunB92.java:84 | playSoundAtEntity | hbm:weapon.sparkShoot | 5.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/GunB92.java:104 | playSoundAtEntity | hbm:weapon.b92Reload | 2F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/GunB92.java:114 | playSoundEffect | random.explode | 100.0f | world.rand.nextFloat() * 0.1F + 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/ItemAmmoArty.java:273 | playSoundEffect | hbm:weapon.explosionMedium | 20.0F | 0.9F + rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/ItemAmmoHIMARS.java:187 | playSoundEffect | hbm:weapon.explosionMedium | 20.0F | 0.9F + rocket.worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/ItemAmmoHIMARS.java:211 | playSoundEffect | hbm:weapon.explosionMedium | 20.0F | 0.9F + rocket.worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/ItemAmmoHIMARS.java:219 | playSoundEffect | hbm:weapon.explosionMedium | 20.0F | 0.9F + rocket.worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/ItemCrucible.java:53 | playSoundEffect | hbm:weapon.cDeploy | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/ItemCrucible.java:84 | playSoundEffect | mob.zombie.woodbreak | 1.0F | 0.75F + victim.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/ItemCrucible.java:178 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:weapon.cSwing"), pitchProbablyIDontFuckingCare) |  |  | not_ported |
| src/main/java/com/hbm/items/weapon/ItemDisperser.java:29 | playSoundAtEntity | random.bow | 0.5F | 0.4F / (itemRand.nextFloat() * 0.4F + 0.8F) | not_ported |
| src/main/java/com/hbm/items/weapon/ItemGenericGrenade.java:24 | playSoundAtEntity | random.bow | 0.5F | 0.4F / (itemRand.nextFloat() * 0.4F + 0.8F) | not_ported |
| src/main/java/com/hbm/items/weapon/WeaponizedCell.java:27 | playSoundEffect | random.explode | 100.0F | world.rand.nextFloat() * 0.1F + 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/grenade/ItemGrenadeFilling.java:232 | playSoundEffect | random.explode | 100.0F | grenade.worldObj.rand.nextFloat() * 0.1F + 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/grenade/ItemGrenadeFilling.java:251 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/grenade/ItemGrenadeFilling.java:263 | playSoundEffect | hbm:entity.ufoBlast | 5.0F | 0.9F + grenade.worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/grenade/ItemGrenadeFilling.java:264 | playSoundEffect | fireworks.blast | 5.0F | 0.5F | not_ported |
| src/main/java/com/hbm/items/weapon/grenade/ItemGrenadeUniversal.java:113 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/grenade/ItemGrenadeUniversal.java:116 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/grenade/ItemGrenadeUniversal.java:117 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/grenade/ItemGrenadeUniversal.java:120 | playSoundAtEntity | hbm:weapon.reload.grenadeTech | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/grenade/ItemGrenadeUniversal.java:123 | playSoundAtEntity | hbm:weapon.reload.grenadeNuka | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/BulletConfig.java:209 | playSoundAtEntity | hbm:weapon.ricochet | 0.25F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/ConfettiUtil.java:42 | playSoundEffect | hbm:weapon.fire.disintegration | 2.0F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/ConfettiUtil.java:49 | playSoundEffect | hbm:weapon.fire.disintegration | 2.0F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/ConfettiUtil.java:79 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/GunStateDecider.java:119 | playSoundEffect | rec.getFireSound(stack) | rec.getFireVolume(stack) | rec.getFirePitch(stack) | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Lego.java:98 | playSoundEffect | rec.getFireSound(stack) | rec.getFireVolume(stack) | rec.getFirePitch(stack) | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Lego.java:131 | playSoundEffect | hbm:weapon.switchmode1 | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Lego.java:133 | playSoundEffect | hbm:weapon.switchmode2 | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:51 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:52 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:53 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:54 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:65 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:68 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:69 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:72 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:73 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:84 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:85 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:89 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.6F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:92 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:93 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.6F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:96 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:99 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:100 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.6F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:111 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:112 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:113 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:117 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:120 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:121 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:124 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:125 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:128 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:129 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:140 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:141 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:142 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:146 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:149 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:150 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:153 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:154 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:157 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:158 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:170 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:171 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:174 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:177 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:178 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:181 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:182 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:183 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:184 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:192 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:195 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:196 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:208 | playSoundAtEntity | hbm:weapon.reload.openLatch | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:218 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:219 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:223 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:224 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:225 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:228 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:229 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:232 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:233 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:245 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:246 | playSoundAtEntity | hbm:weapon.reload.shotgunReload | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:249 | playSoundAtEntity | hbm:weapon.reload.shotgunReload | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:252 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.7F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:255 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.7F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:256 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:257 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:265 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:268 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:269 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:281 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:282 | playSoundAtEntity | hbm:weapon.reload.shotgunReload | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:285 | playSoundAtEntity | hbm:weapon.reload.shotgunReload | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:288 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.7F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:291 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.7F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:292 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:293 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:301 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:304 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:305 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:323 | playSoundAtEntity | hbm:weapon.reload.leverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:338 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:347 | playSoundAtEntity | hbm:weapon.reload.insertCanister | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:348 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:351 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:352 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:355 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:358 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:359 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:370 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:371 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:372 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:373 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:384 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:387 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:388 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:391 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:392 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:411 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:412 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:415 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:416 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:419 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:422 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:423 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:426 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:427 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:447 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:448 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:451 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:452 | playSoundAtEntity | hbm:weapon.reload.impact | 0.25F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:453 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:454 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:457 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:460 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:461 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:472 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:473 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:476 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:477 | playSoundAtEntity | hbm:weapon.reload.impact | 0.25F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:478 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:479 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:482 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:483 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:486 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:487 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:502 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:509 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:512 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:515 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:518 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:519 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:520 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:523 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:526 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:534 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:554 | playSoundAtEntity | hbm:weapon.glReload | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:557 | playSoundAtEntity | hbm:weapon.glOpen | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:558 | playSoundAtEntity | hbm:weapon.glClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:573 | getLoopedSound | hbm:weapon.fire.flameLoop | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:595 | playSoundAtEntity | hbm:weapon.reload.openLatch | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:596 | playSoundAtEntity | hbm:weapon.reload.impact | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:597 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:598 | playSoundAtEntity | hbm:weapon.reload.insertCanister | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:599 | playSoundAtEntity | hbm:weapon.reload.pressureValve | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:610 | playSoundAtEntity | hbm:weapon.reload.openLatch | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:611 | playSoundAtEntity | hbm:weapon.reload.impact | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:612 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:613 | playSoundAtEntity | hbm:weapon.reload.insertCanister | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:614 | playSoundAtEntity | hbm:weapon.reload.pressureValve | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:633 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:634 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:638 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:639 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:640 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:643 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:644 | playSoundAtEntity | hbm:weapon.reload.impact | 0.5F | 1.6F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:645 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:657 | playSoundAtEntity | hbm:weapon.reload.openLatch | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:667 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:668 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:672 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:673 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:674 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:677 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:678 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:689 | playSoundAtEntity | hbm:weapon.reload.openLatch | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:700 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:701 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:705 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:706 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:707 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:710 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:711 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:724 | playSoundAtEntity | hbm:weapon.reload.shotgunCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:731 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:732 | playSoundAtEntity | hbm:weapon.reload.shotgunCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:737 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:738 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:740 | playSoundAtEntity | hbm:weapon.reload.shotgunReload | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:743 | playSoundAtEntity | hbm:weapon.reload.shotgunReload | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:746 | playSoundAtEntity | hbm:weapon.reload.shotgunCockOpen | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:747 | playSoundAtEntity | hbm:weapon.reload.shotgunCockClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:750 | playSoundAtEntity | hbm:weapon.foley.gunWhack | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:751 | playSoundAtEntity | hbm:weapon.foley.gunWhack | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:752 | playSoundAtEntity | hbm:weapon.reload.shotgunCockClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:765 | playSoundAtEntity | hbm:weapon.reload.insertCanister | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:784 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:785 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:789 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:790 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:791 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:792 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:795 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:796 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:797 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:798 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:801 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:802 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:822 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:823 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:827 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:828 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:829 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:830 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:833 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:834 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:835 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:836 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:839 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:840 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:860 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:861 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:865 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:866 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:867 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:868 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:871 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:872 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:875 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:876 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:877 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:878 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:892 | getLoopedSound | hbm:weapon.fire.lockon | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:911 | playSoundAtEntity | hbm:weapon.reload.insertCanister | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:934 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:938 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.65F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:939 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:940 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:941 | playSoundAtEntity | hbm:weapon.reload.impact | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:942 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:943 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:944 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.65F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:955 | playSoundAtEntity | hbm:weapon.reload.impact | 0.5F | 1.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:971 | getLoopedSound | hbm:weapon.fire.flameLoop | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1001 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 0.5F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1002 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 0.5F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1007 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1008 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1019 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1020 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1021 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1025 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1026 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1027 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1028 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1032 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1033 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1034 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1035 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1039 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1040 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1052 | playSoundAtEntity | hbm:turret.howard_reload | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1072 | playSoundAtEntity | hbm:weapon.fire.shredderCycle | 0.25F | 1.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1075 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1076 | playSoundAtEntity | hbm:weapon.fire.shredderCycle | 0.25F | 1.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1079 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1080 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1083 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1084 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1110 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1113 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1114 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1115 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1116 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1117 | playSoundAtEntity | hbm:weapon.reload.impact | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1118 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1119 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1120 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1121 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1127 | playSoundAtEntity | hbm:player.gulp | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1128 | playSoundAtEntity | hbm:player.gulp | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1129 | playSoundAtEntity | hbm:player.gulp | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1130 | playSoundAtEntity | hbm:player.gulp | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1131 | playSoundAtEntity | hbm:player.groan | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1150 | playSoundAtEntity | hbm:weapon.reload.insertCanister | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1170 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1173 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1174 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1177 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1180 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1200 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1203 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1204 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1207 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1210 | playSoundAtEntity | hbm:weapon.reload.revolverSpin | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1224 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1227 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1228 | playSoundAtEntity | hbm:weapon.reload.insertCanister | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1229 | playSoundAtEntity | hbm:weapon.reload.boltClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1233 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1234 | playSoundAtEntity | hbm:weapon.reload.boltClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1245 | playSoundAtEntity | hbm:weapon.fire.shredderCycle | 0.25F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1248 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1249 | playSoundAtEntity | hbm:weapon.fire.shredderCycle | 0.25F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1252 | playSoundAtEntity | hbm:block.squeakyToy | 0.25F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1266 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1270 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1271 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1272 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1273 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1277 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1278 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1279 | playSoundAtEntity | hbm:weapon.reload.impact | 0.25F | 1.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1297 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 0.25F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1300 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1301 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1302 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 0.25F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1305 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1306 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1307 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1308 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1311 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1312 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1314 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1315 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1324 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 0.25F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1327 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1328 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1329 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 0.25F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1332 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1333 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1334 | playSoundAtEntity | hbm:weapon.reload.impact | 0.25F | 1.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1335 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1336 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1339 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1340 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1342 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1343 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1358 | getLoopedSound | hbm:weapon.fire.tauLoop | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1381 | playSoundAtEntity | hbm:weapon.fire.tau | 0.5F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1385 | playSoundAtEntity | hbm:weapon.fire.tau | 0.5F | 0.7F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1405 | playSoundEffect | hbm:entity.ufoBlast | 5.0F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1406 | playSoundEffect | fireworks.blast | 5.0F | 0.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1432 | playSoundAtEntity | hbm:weapon.reload.fatmanFull | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1446 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1450 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1451 | playSoundAtEntity | hbm:weapon.reload.impact | 0.25F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1452 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1453 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1457 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1458 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1459 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1463 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1464 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1465 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1479 | playSoundAtEntity | hbm:weapon.coilgunReload | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1493 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1498 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1499 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1500 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1501 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1520 | playSoundAtEntity | hbm:weapon.fire.smack | 1F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1524 | playSoundEffect | b.stepSound.getStepResourcePath() | 2F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1531 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1532 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1533 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1534 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1553 | playSoundAtEntity | hbm:weapon.reload.magRemove | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1554 | playSoundAtEntity | hbm:weapon.reload.magInsert | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1565 | playSoundAtEntity | hbm:weapon.reload.screw | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1566 | playSoundAtEntity | hbm:weapon.reload.insertRocket | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1567 | playSoundAtEntity | hbm:weapon.reload.screw | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1578 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1579 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 0.9F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1580 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1591 | playSoundAtEntity | hbm:weapon.reload.revolverCock | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1592 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.8F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1598 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1610 | playSoundAtEntity | hbm:weapon.reload.magSmallRemove | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1611 | playSoundAtEntity | hbm:weapon.reload.magSmallInsert | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1612 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1625 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1626 | playSoundAtEntity | hbm:weapon.reload.pistolCock | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1638 | playSoundAtEntity | hbm:weapon.reload.openLatch | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1639 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1644 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1645 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1656 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1657 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1658 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1662 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1663 | playSoundAtEntity | hbm:weapon.reload.rifleCock | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1664 | playSoundAtEntity | hbm:weapon.reload.boltClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1668 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1669 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1670 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1671 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1675 | playSoundAtEntity | hbm:weapon.reload.boltOpen | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1676 | playSoundAtEntity | hbm:weapon.reload.boltClose | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1687 | playSoundAtEntity | hbm:weapon.reload.pressureValve | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1699 | playSoundAtEntity | hbm:weapon.reload.dryFireClick | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1703 | playSoundAtEntity | hbm:weapon.reload.insertRocket | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1704 | playSoundAtEntity | hbm:weapon.reload.boltClose | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1722 | getLoopedSound | electric ? NTMSounds.TURBINE_LARGE_LOOP : NTMSounds.ENGINE_LOOP | (float) speed | (float) speed | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1747 | playSoundAtEntity | hbm:weapon.reload.openLatch | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1748 | playSoundAtEntity | hbm:weapon.reload.impact | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1749 | playSoundAtEntity | hbm:weapon.reload.revolverClose | 1F | 0.75F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1750 | playSoundAtEntity | hbm:weapon.reload.insertCanister | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/Orchestras.java:1751 | playSoundAtEntity | hbm:weapon.reload.pressureValve | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactory10ga.java:117 | playSoundEffect | rec.getFireSound(stack) | rec.getFireVolume(stack) | rec.getFirePitch(stack) | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactory12ga.java:103 | playSoundEffect | hbm:weapon.boat | 100F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactory12ga.java:452 | playSoundEffect | rec.getFireSound(stack) | rec.getFireVolume(stack) | rec.getFirePitch(stack) * (timeFired > 1 ? 0.9F : 1F) | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactory40mm.java:146 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactory44.java:77 | playSoundEffect | hbm:alarm.trainHorn | 100F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactory9mm.java:169 | playSoundEffect | primary.getFireSound(stack) | primary.getFireVolume(stack) | primary.getFirePitch(stack) | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryAccelerator.java:162 | playSoundEffect | hbm:weapon.fire.tauRelease | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryAccelerator.java:218 | playSoundAtEntity | random.orb | 1.0F | 1F + player.getRNG().nextFloat() * 0.25F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryCatapult.java:117 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryCatapult.java:135 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryCatapult.java:154 | playSoundEffect | hbm:weapon.mukeExplosion | 15.0F | 1.0F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryDrill.java:127 | playSoundAtEntity | random.break | 0.5F | 0.8F + world.rand.nextFloat() * 0.6F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryEnergy.java:96 | playSoundEffect | hbm:entity.ufoBlast | 5.0F | 0.9F + beam.worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryEnergy.java:97 | playSoundEffect | fireworks.blast | 5.0F | 0.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryTool.java:91 | playSoundEffect | random.fizz | 1.0F | 1.5F + bullet.worldObj.rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryTool.java:147 | playSoundEffect | random.fizz | 1.0F | 1.5F + bullet.worldObj.rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/factory/XFactoryTool.java:185 | playSoundEffect | random.fizz | 1.0F | 1.5F + bullet.worldObj.rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/impl/ItemGunNI4NI.java:46 | playSoundAtEntity | hbm:item.techBoop | 1.0F | 1F + newCount / (float) maxCoin | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/impl/ItemGunStinger.java:62 | playSoundAtEntity | hbm:item.techBleep | 1F | 1F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/mods/WeaponModCarbineBayonet.java:57 | playSoundAtEntity | hbm:weapon.fire.stab | 1F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/mods/WeaponModCarbineBayonet.java:61 | playSoundEffect | b.stepSound.getStepResourcePath() | 2F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/mods/WeaponModMASBayonet.java:57 | playSoundAtEntity | hbm:weapon.fire.stab | 1F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/items/weapon/sedna/mods/WeaponModMASBayonet.java:61 | playSoundEffect | b.stepSound.getStepResourcePath() | 2F | 0.9F + entity.getRNG().nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/main/ClientProxy.java:2040 | playSound | new PositionedSoundRecord(new ResourceLocation(sound), volume, pitch, (float) x, (float) y, (float) z) |  |  | not_ported |
| src/main/java/com/hbm/main/ModEventHandler.java:793 | playSoundAtEntity | hbm:weapon.shotgunShoot | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/main/ModEventHandler.java:981 | playSoundAtEntity | hbm:item.syringe | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/main/ModEventHandlerClient.java:677 | playSound | sounds |  |  | not_ported |
| src/main/java/com/hbm/packet/toclient/TESirenPacket.java:81 | playSound | s |  |  | not_ported |
| src/main/java/com/hbm/packet/toclient/TESirenPacket.java:93 | playSound | new SoundLoopSiren(TrackType.getEnum(m.id).getSoundLocation(), te, TrackType.getEnum(m.id).getType()) |  |  | not_ported |
| src/main/java/com/hbm/packet/toserver/AuxButtonPacket.java:169 | playSoundAtEntity | hbm:entity.ducc | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/particle/ParticleSkeleton.java:108 | playSoundClient | mob.skeleton.hurt | 0.25F | 0.8F + rand.nextFloat() * 0.4F | not_ported |
| src/main/java/com/hbm/particle/ParticleSpentCasing.java:233 | playSoundClient | this.config.getSound() | this.config.PLINK_LARGE.equals(this.config.getSound()) ? 1F : 0.5F | 1F + this.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/potion/HbmPotion.java:137 | playSoundEffect | hbm:weapon.laserBang | 100.0F | 1.0F | not_ported |
| src/main/java/com/hbm/qmaw/GuiQMAW.java:208 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("hbm:alarm.singer"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/qmaw/GuiQMAW.java:220 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/qmaw/GuiQMAW.java:237 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/qmaw/components/QComponentLink.java:97 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/render/item/weapon/sedna/ItemRenderFolly.java:108 | playSoundClient | hbm:weapon.fire.vstar | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/saveddata/satellites/SatelliteResonator.java:30 | playSoundEffect | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/saveddata/satellites/SatelliteResonator.java:35 | playSoundEffect | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/sound/AudioDynamic.java:72 | playSound | this |  |  | not_ported |
| src/main/java/com/hbm/tileentity/DoorDecl.java:143 | playSoundEffect | hbm:block.vaultScrapeNew | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/DoorDecl.java:145 | playSoundEffect | hbm:block.vaultThudNew | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/DoorDecl.java:147 | playSoundEffect | hbm:block.vaultScrapeNew | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/DoorDecl.java:149 | playSoundEffect | hbm:block.vaultThudNew | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityDoorGeneric.java:251 | getLoopedSound | doorType.getOpenSoundLoop() | getVolume() | 1F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityDoorGeneric.java:256 | playSound | doorType.getOpenSoundStart() | getVolume() | 1F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityDoorGeneric.java:262 | getLoopedSound | doorType.getSoundLoop2() | getVolume() | 1F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityDoorGeneric.java:273 | getLoopedSound | doorType.getCloseSoundLoop() | getVolume() | 1F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityDoorGeneric.java:278 | playSound | doorType.getCloseSoundStart() | getVolume() | 1F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityDoorGeneric.java:284 | getLoopedSound | doorType.getSoundLoop2() | getVolume() | 1F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityDoorGeneric.java:302 | playSound | doorType.getOpenSoundEnd() | getVolume() | 1F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityDoorGeneric.java:308 | playSound | doorType.getCloseSoundEnd() | getVolume() | 1F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityLoadedBase.java:247 | playSoundEffect | hbm:block.metalImpact | 3F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/TileEntityMachinePolluting.java:66 | playSoundEffect | random.fizz | 0.1F | 1.5F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityCharge.java:22 | playSoundEffect | hbm:weapon.fstbmbPing | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityCompactLauncher.java:315 | playSoundEffect | hbm:weapon.missileTakeOff | 10.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityFireworks.java:42 | playSoundAtEntity | hbm:weapon.rocketFlame | 3.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityLandmine.java:78 | playSoundEffect | hbm:weapon.fstbmbStart | 3.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityLaunchPadBase.java:361 | playSoundEffect | hbm:weapon.missileTakeOff | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityLaunchPadLarge.java:154 | playSoundEffect | hbm:door.wgh_stop | 2F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityLaunchPadLarge.java:155 | playSoundEffect | hbm:door.garage_stop | 2F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityLaunchPadLarge.java:172 | getLoopedSound | hbm:door.wgh_start | 0.75F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityLaunchPadLarge.java:185 | getLoopedSound | hbm:door.garage_move | 1.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityLaunchPadRusted.java:117 | playSoundEffect | hbm:weapon.missileTakeOff | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityLaunchTable.java:342 | playSoundEffect | hbm:weapon.missileTakeOff | 10.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/bomb/TileEntityNukeBalefire.java:52 | playSoundEffect | hbm:weapon.fstbmbPing | 5.0F | 1.0F | crates/ntm-core/src/blast/visuals.rs, crates/ntm-core/src/bomb.rs |
| src/main/java/com/hbm/tileentity/bomb/TileEntityNukeBalefire.java:84 | playSoundEffect | hbm:weapon.fstbmbStart | 5.0F | 1.0F | crates/ntm-core/src/blast/visuals.rs, crates/ntm-core/src/bomb.rs |
| src/main/java/com/hbm/tileentity/deco/TileEntityLanternBehemoth.java:41 | playSoundEffect | hbm:block.hornNearSingle | 10F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/deco/TileEntityLanternBehemoth.java:42 | playSoundEffect | hbm:block.hornFarSingle | 10000F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/deco/TileEntityLanternBehemoth.java:43 | playSoundEffect | hbm:block.hornNearDual | 10F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/deco/TileEntityLanternBehemoth.java:44 | playSoundEffect | hbm:block.hornFarDual | 10000F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/deco/TileEntityTrappedBrick.java:100 | playSoundEffect | random.click | 0.3F | 0.6F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityBlastDoor.java:91 | playSoundEffect | hbm:block.reactorStart | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityBlastDoor.java:98 | playSoundEffect | hbm:block.reactorStop | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityBlastDoor.java:107 | playSoundEffect | hbm:block.reactorStart | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityBlastDoor.java:114 | playSoundEffect | hbm:block.reactorStop | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityBroadcaster.java:78 | getLoopedSound | "hbm:block.broadcast" + (rand.nextInt(3) + 1) | 25F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityCharger.java:80 | playSoundEffect | random.fizz | 0.2F | 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityCharger.java:91 | playSoundEffect | tile.piston.out | 0.5F | 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityCharger.java:96 | playSoundEffect | tile.piston.in | 0.5F | 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityChungus.java:137 | getLoopedSound | hbm:block.chungusTurbineRunning | 1.0F | 1.0F | crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs, crates/ntm-core/src/steam.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityConveyorPress.java:151 | playSoundEffect | hbm:block.pressOperate | getVolume(1.5F) | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityCore.java:119 | playSoundEffect | random.explode | 100000.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityCoreEmitter.java:130 | playSoundEffect | random.fizz | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityFEL.java:179 | playSoundEffect | random.fizz | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityFEL.java:186 | playSoundEffect | random.fizz | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityFEL.java:296 | getLoopedSound | hbm:block.fel | 2.0F | 2.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityFF.java:89 | playSoundAtEntity | hbm:weapon.sparkShoot | 2.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityFF.java:114 | playSoundAtEntity | hbm:weapon.sparkShoot | 2.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityFireboxBase.java:115 | playSoundEffect | fire.fire | 1.0F | 0.5F + worldObj.rand.nextFloat() * 0.5F | crates/ntm-core/src/foundry/firebox.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityForceField.java:431 | playSoundAtEntity | hbm:weapon.sparkShoot | 2.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityForceField.java:461 | playSoundAtEntity | hbm:weapon.sparkShoot | 2.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityFurnaceCombination.java:112 | playSoundEffect | hbm:weapon.flamethrowerShoot | 0.25F | 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityFurnaceIron.java:100 | playSoundEffect | fire.fire | 1.0F | 0.5F + worldObj.rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityGeiger.java:59 | playSoundEffect | "hbm:item.geiger" + r | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityGeiger.java:62 | playSoundEffect | "hbm:item.geiger"+ (1 + worldObj.rand.nextInt(1)) | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityHeatBoiler.java:130 | getLoopedSound | hbm:block.boiler | 0.125F | 1.0F | crates/ntm-core/src/boiler.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityHeatBoiler.java:230 | playSoundEffect | hbm:block.boilerGroan | 0.5F | 1.0F | crates/ntm-core/src/boiler.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityHeatBoilerIndustrial.java:119 | getLoopedSound | hbm:block.boiler | 0.125F | 1.0F | crates/ntm-core/src/boiler.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityHeatBoilerIndustrial.java:216 | playSoundEffect | hbm:block.boilerGroan | 0.5F | 1.0F | crates/ntm-core/src/boiler.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityHeaterElectric.java:94 | getLoopedSound | hbm:block.electricHum | 0.25F | 1.0F | crates/ntm-core/src/heater.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:191 | playSoundClient | hbm:door.sliding_seal_stop | 25F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:222 | playSoundClient | hbm:door.garage_stop | 35F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:223 | playSoundClient | hbm:door.wgh_big_stop | 25F | 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:224 | playSoundClient | hbm:door.wgh_stop | 25F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:225 | playSoundClient | hbm:door.wgh_big_stop | 25F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:234 | getLoopedSound | hbm:door.garage_move | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:235 | getLoopedSound | hbm:door.wgh_start | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:236 | getLoopedSound | hbm:door.wgh_big_start | 2.0F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:237 | getLoopedSound | hbm:alarm.regularSiren | 10F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadLambda.java:319 | playSoundEffect | hbm:entity.soyuzTakeoff | 100F | 1.1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadSoyuz.java:176 | playSoundEffect | hbm:door.sliding_seal_stop | 35F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadSoyuz.java:266 | playSoundClient | hbm:door.garage_stop | 35F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadSoyuz.java:267 | playSoundClient | hbm:door.wgh_big_stop | 35F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadSoyuz.java:276 | getLoopedSound | hbm:block.soyuzReady | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadSoyuz.java:277 | getLoopedSound | hbm:door.garage_move | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadSoyuz.java:278 | getLoopedSound | hbm:door.wgh_big_start | 2.0F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadSoyuz.java:444 | playSoundEffect | hbm:alarm.hatch | 100F | 1.1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLaunchpadSoyuz.java:446 | playSoundEffect | hbm:entity.soyuzTakeoff | 100F | 1.1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLockableBase.java:93 | playSoundAtEntity | hbm:block.lockOpen | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLockableBase.java:98 | playSoundAtEntity | hbm:block.lockOpen | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLockableBase.java:133 | playSoundAtEntity | hbm:item.pinUnlock | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityLockableBase.java:137 | playSoundAtEntity | hbm:item.pinBreak | 1.0F | 0.8F + player.worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAnnihilator.java:109 | playSoundEffect | hbm:weapon.flamethrowerShoot | getVolume(1F) | 0.5F + worldObj.rand.nextFloat() * 0.25F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineArcFurnaceLarge.java:101 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | crates/ntm-core/src/arc_furnace.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/foundry_server.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineArcFurnaceLarge.java:201 | getLoopedSound | hbm:door.wgh_start | this.getVolume(0.75F) | 1.0F | crates/ntm-core/src/arc_furnace.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/foundry_server.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineArcFurnaceLarge.java:213 | playSoundClient | hbm:door.wgh_stop | this.getVolume(1) | 1F | crates/ntm-core/src/arc_furnace.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/foundry_server.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineArcFurnaceLarge.java:218 | getLoopedSound | hbm:block.electricHum | this.getVolume(1.5F) | 0.75F | crates/ntm-core/src/arc_furnace.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/foundry_server.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineArcWelder.java:71 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAssemblyFactory.java:245 | getLoopedSound | hbm:block.motor | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAssemblyFactory.java:508 | playSoundClient | hbm:block.assemblerStart | getVolume(0.25F) | 1.25F + worldObj.rand.nextFloat() * 0.25F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAssemblyFactory.java:605 | playSoundClient | hbm:block.assemblerCut | getVolume(0.5F) | 1F + rand.nextFloat() * 0.25F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAssemblyFactory.java:609 | playSoundClient | hbm:block.assemblerStrike | getVolume(0.5F) | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAssemblyMachine.java:162 | playSoundClient | hbm:block.assemblerStrike | this.getVolume(0.5F) | 1F | crates/ntm-core/src/crafter.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAssemblyMachine.java:186 | playSoundClient | hbm:block.assemblerStart | this.getVolume(0.25F) | 1.25F + worldObj.rand.nextFloat() * 0.25F | crates/ntm-core/src/crafter.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAssemblyMachine.java:194 | getLoopedSound | hbm:block.motor | 0.5F | 0.75F | crates/ntm-core/src/crafter.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAssemblyMachine.java:230 | playSoundClient | hbm:block.assemblerStop | this.getVolume(0.25F) | 1.5F | crates/ntm-core/src/crafter.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAutosaw.java:140 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineAutosaw.java:283 | getLoopedSound | hbm:block.engine | 1.0F | 1.0F + worldObj.rand.nextFloat() * 0.1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineBlastFurnace.java:132 | playSoundEffect | fire.fire | 1.0F | 0.5F + worldObj.rand.nextFloat() * 0.25F | crates/ntm-core/src/blastfurnace.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineCentrifuge.java:291 | getLoopedSound | hbm:block.centrifugeOperate | 1.0F | 1.0F | crates/ntm-core/src/crafter.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineChemicalFactory.java:293 | getLoopedSound | hbm:block.chemicalPlant | 1F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineChemicalPlant.java:166 | getLoopedSound | hbm:block.chemicalPlant | 1F | 1.0F | crates/ntm-core/src/crafter.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineCombustionEngine.java:176 | getLoopedSound | hbm:block.igeneratorOperate | 1.0F | 1.0F | crates/ntm-core/src/combustion.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineCompressor.java:39 | playSoundClient | hbm:item.boltgun | this.getVolume(0.5F) | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineCrystallizer.java:160 | getLoopedSound | hbm:block.chemicalPlant | 1F | 0.75F | crates/ntm-core/src/sounds.rs, crates/ntm-core/src/crafter.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineCrystallizer.java:369 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | crates/ntm-core/src/sounds.rs, crates/ntm-core/src/crafter.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineCyclotron.java:296 | playSoundEffect | hbm:item.upgradePlug | 1.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineDiesel.java:200 | getLoopedSound | hbm:block.engine | 1.0F | 1.0F | crates/ntm-core/src/combustion.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineEPress.java:107 | playSoundEffect | hbm:block.pressOperate | getVolume(1.5F) | 1.0F | crates/ntm-core/src/press.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineGasCent.java:270 | getLoopedSound | hbm:block.centrifugeOperate | 1.0F | 1.0F | crates/ntm-core/src/gascent.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/fluid_server.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineHephaestus.java:99 | getLoopedSound | hbm:block.hephaestusRunning | 0.75F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineIndustrialTurbine.java:117 | getLoopedSound | hbm:block.largeTurbineRunning | volume | pitch | crates/ntm-core/src/steam.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineIntake.java:85 | getLoopedSound | hbm:block.motor | 0.25F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineLargeTurbine.java:140 | getLoopedSound | hbm:block.largeTurbineRunning | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineMiningLaser.java:335 | playSoundEffect | hbm:block.screm | 2000.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineMiningLaser.java:576 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineMissileAssembly.java:304 | playSoundEffect | hbm:block.missileAssembly2 | 1F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineOreSlopper.java:174 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | crates/ntm-core/src/bedrock.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachinePUREX.java:158 | getLoopedSound | hbm:block.chemicalPlant | 1F | 0.75F | crates/ntm-core/src/crafter.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachinePrecAss.java:160 | playSoundClient | hbm:block.assemblerStrike | this.getVolume(0.5F) | 1.25F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachinePrecAss.java:186 | playSoundClient | hbm:block.assemblerStart | this.getVolume(0.25F) | 1.25F + worldObj.rand.nextFloat() * 0.25F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachinePrecAss.java:209 | playSoundClient | hbm:block.assemblerStop | this.getVolume(0.25F) | 1.25F + worldObj.rand.nextFloat() * 0.25F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachinePrecAss.java:246 | getLoopedSound | hbm:block.motor | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachinePress.java:100 | playSoundEffect | hbm:block.pressOperate | getVolume(1.5F) | 1.0F | crates/ntm-core/src/press.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachinePumpBase.java:105 | playSoundClient | hbm:block.steamEngineOperate | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachinePumpBase.java:106 | playSoundClient | game.neutral.swim.splash | 1F | 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineRadarNT.java:174 | playSoundEffect | hbm:block.sonarPing | 5.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineRadarNT.java:498 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineRadarNT.java:514 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineRadarNT.java:521 | playSoundAtEntity | hbm:item.techBleep | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineRockMill.java:123 | playSoundEffect | sound | this.getVolume(1.0F) | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineShredder.java:262 | playSoundEffect | minecart.base | getVolume(1.0F) | 0.75F | crates/ntm-core/src/shredder.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineSolderingStation.java:79 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineTeleporter.java:122 | playSoundEffect | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineTeleporter.java:154 | playSoundEffect | mob.endermen.portal | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineThresher.java:195 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineThresher.java:261 | getLoopedSound | hbm:block.engine | 1.0F | 1.0F + worldObj.rand.nextFloat() * 0.1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineTurbineGas.java:167 | getLoopedSound | hbm:block.turbinegasRunning | getVolume(1.0F) | 2.0F | crates/ntm-core/src/combustion.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineTurbineGas.java:172 | getLoopedSound | hbm:block.turbinegasRunning | getVolume(1.0F) | 2.0F | crates/ntm-core/src/combustion.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineTurbineGas.java:298 | playSoundEffect | hbm:block.turbinegasStartup | getVolume(1.0F) | 1.0F | crates/ntm-core/src/combustion.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineTurbineGas.java:323 | playSoundEffect | hbm:block.turbinegasShutdown | getVolume(1.0F) | 1.0F | crates/ntm-core/src/combustion.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineTurbofan.java:243 | playSoundEffect | hbm:block.damage | 3.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | crates/ntm-core/src/combustion.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineTurbofan.java:306 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | crates/ntm-core/src/combustion.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityMachineTurbofan.java:438 | getLoopedSound | hbm:block.turbofanOperate | 1.0F | 1.0F | crates/ntm-core/src/combustion.rs, crates/ntm-core/src/sounds.rs, crates/ntm-core/src/server/container_bridge.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityPWRController.java:348 | getLoopedSound | hbm:block.reactorLoop | 1F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityRadioRec.java:48 | playSoundEffect | "note." + s | 3.0F | f | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityReactorZirnox.java:414 | playSoundEffect | hbm:block.rbmk_explosion | 10.0F | 1.0F | crates/ntm-core/src/server/zirnox_server.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityRefueler.java:91 | playSoundEffect | random.fizz | 0.2F | 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntitySawmill.java:98 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntitySawmill.java:120 | playSoundEffect | hbm:block.warnOverspeed | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntitySoyuzLauncher.java:82 | playSoundEffect | hbm:alarm.hatch | 100F | 1.1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntitySoyuzLauncher.java:154 | getLoopedSound | hbm:block.soyuzReady | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntitySoyuzLauncher.java:217 | playSoundEffect | hbm:entity.soyuzTakeoff | 100F | 1.1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntitySteamEngine.java:118 | playSoundEffect | hbm:block.steamEngineOperate | getVolume(1.0F) | 0.5F + (acceleration / 80F) | crates/ntm-core/src/server/container_bridge.rs, crates/ntm-core/src/steam.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityStirling.java:87 | playSoundEffect | hbm:block.warnOverspeed | 2.0F | 1.0F | crates/ntm-core/src/solar_stirling.rs, crates/ntm-core/src/sounds.rs |
| src/main/java/com/hbm/tileentity/machine/TileEntityTesla.java:121 | playSoundAtEntity | hbm:weapon.tesla | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityWatz.java:198 | playSoundEffect | hbm:block.rbmk_explosion | 50.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/TileEntityZirnoxDestroyed.java:53 | playSoundEffect | fire.fire | 1.0F + worldObj.rand.nextFloat() | worldObj.rand.nextFloat() * 0.7F + 0.3F | crates/ntm-core/src/server/zirnox_server.rs |
| src/main/java/com/hbm/tileentity/machine/fusion/TileEntityFusionBoiler.java:106 | playSoundEffect | hbm:block.boilerGroan | 2.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/fusion/TileEntityFusionKlystron.java:146 | getLoopedSound | hbm:block.fel | getVolume(speed) | speed | not_ported |
| src/main/java/com/hbm/tileentity/machine/fusion/TileEntityFusionKlystronCreative.java:60 | getLoopedSound | hbm:block.fel | getVolume(speed) | speed | not_ported |
| src/main/java/com/hbm/tileentity/machine/fusion/TileEntityFusionMHDT.java:129 | getLoopedSound | hbm:block.largeTurbineRunning | getVolume(speed) | speed | not_ported |
| src/main/java/com/hbm/tileentity/machine/fusion/TileEntityFusionPlasmaForge.java:455 | playSoundClient | hbm:item.boltgun | getVolume(0.25F) | 1.25F | not_ported |
| src/main/java/com/hbm/tileentity/machine/fusion/TileEntityFusionTorus.java:266 | getLoopedSound | hbm:block.fusionReactorRunning | getVolume(speed) | speed | not_ported |
| src/main/java/com/hbm/tileentity/machine/oil/TileEntityMachineGasFlare.java:147 | playSoundEffect | random.fizz | getVolume(1.5F) | 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/machine/oil/TileEntityMachineGasFlare.java:183 | playSoundEffect | hbm:weapon.flamethrowerShoot | getVolume(1.5F) | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/oil/TileEntityMachineOilWell.java:95 | playSoundEffect | game.neutral.swim.splash | 2.0F | 0.5F | crates/ntm-core/src/oil_drill.rs, crates/ntm-core/src/server/oil_server.rs |
| src/main/java/com/hbm/tileentity/machine/oil/TileEntityMachinePyroOven.java:105 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | crates/ntm-core/src/crafter.rs |
| src/main/java/com/hbm/tileentity/machine/oil/TileEntityMachinePyroOven.java:326 | getLoopedSound | hbm:block.pyroOperate | 1.0F | 1.0F | crates/ntm-core/src/crafter.rs |
| src/main/java/com/hbm/tileentity/machine/oil/TileEntityMachineRefinery.java:228 | getLoopedSound | hbm:block.boiler | 0.25F | 1.0F | crates/ntm-core/src/processing.rs, crates/ntm-core/src/refinery.rs |
| src/main/java/com/hbm/tileentity/machine/oil/TileEntityMachineVacuumDistill.java:116 | getLoopedSound | hbm:block.boiler | 0.25F | 1.0F | crates/ntm-core/src/processing.rs |
| src/main/java/com/hbm/tileentity/machine/oil/TileEntityOilDrillBase.java:182 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | crates/ntm-core/src/oil_drill.rs, crates/ntm-core/src/server/oil_server.rs |
| src/main/java/com/hbm/tileentity/machine/pile/TileEntityPileCore.java:193 | playSoundEffect | random.break | 1F | 0.75F | crates/ntm-core/src/server/pile_server.rs, crates/ntm-core/src/pile.rs |
| src/main/java/com/hbm/tileentity/machine/pile/TileEntityPileCore.java:234 | playSoundEffect | random.break | 1F | 1.25F | crates/ntm-core/src/server/pile_server.rs, crates/ntm-core/src/pile.rs |
| src/main/java/com/hbm/tileentity/machine/pile/TileEntityPileLoader.java:95 | playSoundEffect | hbm:weapon.reload.boltOpen | this.getVolume(1F) | 1F | crates/ntm-core/src/server/pile_server.rs |
| src/main/java/com/hbm/tileentity/machine/pile/TileEntityPileLoader.java:106 | playSoundEffect | hbm:weapon.reload.boltOpen | this.getVolume(1F) | 0.75F | crates/ntm-core/src/server/pile_server.rs |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKAutoloader.java:144 | getLoopedSound | hbm:door.wgh_start | this.getVolume(0.75F) | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKAutoloader.java:153 | playSoundClient | hbm:door.wgh_stop | this.getVolume(2F) | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKBase.java:551 | playSoundEffect | hbm:block.rbmk_explosion | 50.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKBoiler.java:116 | playSoundEffect | hbm:block.steamEngineOperate | 2F | 1F + worldObj.rand.nextFloat() * 0.25F | not_ported |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKConsole.java:809 | playSoundEffect | hbm:block.shutdown | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKKeyPad.java:118 | playSoundEffect | random.click | 1F | this.isPressed ? 1F : 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKLever.java:94 | playSoundEffect | hbm:block.leverStart | 1F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKLever.java:118 | playSoundEffect | hbm:block.leverStop | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKLever.java:130 | playSoundEffect | hbm:block.leverStop | 0.5F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/rbmk/TileEntityRBMKLever.java:136 | playSoundEffect | hbm:block.spark | 1F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityBarrel.java:175 | playSoundEffect | random.fizz | 1.0F | 1.0F | crates/ntm-core/src/fluidbox.rs, crates/ntm-core/src/machine.rs, crates/ntm-core/src/native.rs |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityBatteryREDD.java:81 | getLoopedSound | hbm:block.fensuHum | this.getVolume(1.5F) | pitch | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityBatterySocket.java:69 | playSoundEffect | hbm:entity.ufoBlast | 5.0F | 0.9F + world.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityCrateBase.java:86 | playSoundEffect | hbm:block.crateOpen | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityCrateBase.java:91 | playSoundEffect | hbm:block.crateClose | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityFileCabinet.java:80 | playSoundEffect | hbm:block.crateOpen | 0.8F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityFileCabinet.java:83 | playSoundEffect | hbm:block.crateOpen | 0.5F | this.worldObj.rand.nextFloat() * 0.1F + 0.7F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityFileCabinet.java:86 | playSoundEffect | hbm:block.crateOpen | 0.5F | this.worldObj.rand.nextFloat() * 0.1F + 0.7F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityFileCabinet.java:96 | playSoundEffect | hbm:block.crateClose | 0.8F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityFileCabinet.java:99 | playSoundEffect | hbm:block.crateClose | 0.8F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityMassStorage.java:274 | playSoundEffect | hbm:block.storageOpen | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/machine/storage/TileEntityMassStorage.java:279 | playSoundEffect | hbm:block.storageClose | 0.5F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/network/TileEntityCraneExtractor.java:54 | playSoundEffect | hbm:item.upgradePlug | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/network/TileEntityDroneCrate.java:131 | playSoundEffect | hbm:item.unpack | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/network/TileEntityDroneCrate.java:156 | playSoundEffect | hbm:item.unpack | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/network/TileEntityDroneCrate.java:168 | playSoundEffect | hbm:item.unpack | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/network/TileEntityDroneCrate.java:189 | playSoundEffect | hbm:item.unpack | 0.5F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/network/TileEntityDroneDock.java:124 | playSoundEffect | hbm:block.storageOpen | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/network/TileEntityFluidCounterValve.java:95 | playSoundEffect | hbm:block.reactorStart | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/network/TileEntityRadioTelex.java:120 | playSoundEffect | random.orb | 2F | 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/network/pneumatic/TileEntityPneumoTube.java:127 | playSoundEffect | hbm:weapon.reload.tubeFwoomp | 0.25F | 0.9F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretArty.java:365 | playSoundEffect | hbm:turret.jeremy_fire | 25.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretBaseNT.java:848 | playSoundEffect | hbm:block.openC | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretBaseNT.java:853 | playSoundEffect | hbm:block.closeC | 1.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretChekhov.java:82 | playSoundEffect | hbm:turret.chekhov_fire | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretFritz.java:96 | playSoundEffect | hbm:weapon.flamethrowerShoot | 2F | 1F + worldObj.rand.nextFloat() * 0.5F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretHIMARS.java:307 | playSoundEffect | hbm:weapon.rocketFlame | 25.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretHoward.java:121 | playSoundEffect | hbm:turret.howard_reload | 4.0F | 1F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretHoward.java:139 | playSoundEffect | hbm:turret.howard_fire | 4.0F | 0.9F + worldObj.rand.nextFloat() * 0.3F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretHoward.java:140 | playSoundEffect | hbm:turret.howard_fire | 4.0F | 1F + worldObj.rand.nextFloat() * 0.3F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretHowardDamaged.java:77 | playSoundEffect | hbm:turret.howard_fire | 4.0F | 0.7F + worldObj.rand.nextFloat() * 0.3F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretJeremy.java:78 | playSoundEffect | hbm:turret.jeremy_reload | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretJeremy.java:96 | playSoundEffect | hbm:turret.jeremy_fire | 4.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretMaxwell.java:261 | playSoundEffect | hbm:block.screm | 20.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretMaxwell.java:263 | playSoundEffect | mob.zombie.woodbreak | 2.0F | 0.95F + worldObj.rand.nextFloat() * 0.2F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretRichard.java:138 | playSoundEffect | hbm:turret.richard_fire | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretSentry.java:181 | playSoundEffect | hbm:turret.sentry_fire | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretSentry.java:240 | playSoundAtEntity | hbm:turret.sentry_lockon | 2.0F | 1.5F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretSentryDamaged.java:70 | playSoundEffect | hbm:turret.sentry_fire | 2.0F | 1.0F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretSentryDamaged.java:81 | playSoundEffect | hbm:turret.sentry_fire | 2.0F | 0.75F | not_ported |
| src/main/java/com/hbm/tileentity/turret/TileEntityTurretTauon.java:133 | playSoundEffect | hbm:weapon.tauShoot | 4.0F | 0.9F + worldObj.rand.nextFloat() * 0.3F | not_ported |
| src/main/java/com/hbm/util/EntityDamageUtil.java:172 | playSound | sound | getSoundVolume(living) | getSoundPitch(living) | not_ported |
| src/main/java/com/hbm/util/EntityDamageUtil.java:176 | playSound | sound | getSoundVolume(living) | getSoundPitch(living) | not_ported |
| src/main/java/com/hbm/util/EntityDamageUtil.java:324 | playSound | s | getSoundVolume(living) | getSoundPitch(living) | not_ported |
| src/main/java/com/hbm/util/EntityDamageUtil.java:332 | playSound | s | getSoundVolume(living) | getSoundPitch(living) | not_ported |
| src/main/java/com/hbm/wiaj/GuiWorldInAJar.java:189 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/wiaj/GuiWorldInAJar.java:202 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/wiaj/GuiWorldInAJar.java:210 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/wiaj/GuiWorldInAJar.java:218 | playSound | PositionedSoundRecord.func_147674_a(new ResourceLocation("gui.button.press"), 1.0F) |  |  | not_ported |
| src/main/java/com/hbm/world/gen/util/LogicBlockInteractions.java:89 | playSoundAtEntity | hbm:block.lockOpen | 3.0F | 0.8F | not_ported |
