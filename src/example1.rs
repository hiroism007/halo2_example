use std::{marker::PhantomData};

use halo2_proofs::{arithmetic::FieldExt, circuit::*, plonk::*, poly::Rotation, pasta::Fp, dev::MockProver};


#[derive(Debug, Clone)]
struct ACell<F: FieldExt>(AssignedCell<F, F>);

#[derive(Clone, Debug)]
struct FiboConfig {
    pub advice: [Column<Advice>; 3],
    pub selector: Selector,
    pub instance: Column<Instance>,
}

struct FiboChip<F: FieldExt> {
    config: FiboConfig,
    _marker: PhantomData<F>,
}

impl<F: FieldExt> FiboChip<F> {
    fn construct(config: FiboConfig) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }

    // instance は public inputs 用の column
    fn configure(meta: &mut ConstraintSystem<F>, advice: [Column<Advice>; 3], instance: Column<Instance>,) -> FiboConfig {
        let col_a = advice[0];
        let col_b = advice[1];
        let col_c = advice[2];
        let selector = meta.selector();

        // copy constraint を追加するために enable_equality で有効化する必要がある
        meta.enable_equality(col_a);
        meta.enable_equality(col_b);
        meta.enable_equality(col_c);
        meta.enable_equality(instance);

        meta.create_gate("add", |meta| {
            // col_a | col_b | col_c | selector
            //    a       b      c        s
            let s = meta.query_selector(selector);
            let a = meta.query_advice(col_a, Rotation::cur());
            let b = meta.query_advice(col_b, Rotation::cur());
            let c = meta.query_advice(col_c, Rotation::cur());
            vec![s * (a + b - c)]
        });

        FiboConfig {
            advice: [col_a, col_b, col_c],
            selector,
            instance
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn assign_first_row(
        &self,
        mut layouter: impl Layouter<F>,
        a: Value<F>,
        b: Value<F>,
    ) -> Result<(ACell<F>, ACell<F>, ACell<F>), Error> {
        layouter.assign_region(
            || "first row",
            |mut region| {
                self.config.selector.enable(&mut region, 0)?;

                let a_cell = region
                    .assign_advice(|| "a", self.config.advice[0], 0, || a)
                    .map(ACell)?;

                let b_cell = region
                    .assign_advice(|| "b", self.config.advice[1], 0, || b)
                    .map(ACell)?;

                let c_cell = region
                    .assign_advice(|| "c", self.config.advice[2], 0, || a + b)
                    .map(ACell)?;

                Ok((a_cell, b_cell, c_cell))
            },
        )
    }

    pub fn assign_row(
        &self,
        mut layouter: impl Layouter<F>,
        prev_b: &ACell<F>,
        prev_c: &ACell<F>,
    ) -> Result<ACell<F>, Error> {
        layouter.assign_region(
            || "next row",
            |mut region| {
                self.config.selector.enable(&mut region, 0)?;

                prev_b
                    .0
                    .copy_advice(|| "a", &mut region, self.config.advice[0], 0)?;
                prev_c
                    .0
                    .copy_advice(|| "b", &mut region, self.config.advice[1], 0)?;

                let c_val = prev_b.0.value().copied() + prev_c.0.value();

                let c_cell = region
                    .assign_advice(|| "c", self.config.advice[2], 0, || c_val)
                    .map(ACell)?;

                Ok(c_cell)
            },
        )
    }

    pub fn expose_public(&self,  mut layouter: impl Layouter<F>, cell: &ACell<F>, row: usize) -> Result<(), Error> {
        // cell が instance の row で指定されところと一致する constraint を作成
        layouter.constrain_instance(cell.0.cell(), self.config.instance, row)
    }
}

#[derive(Default)]
struct MyCircuit<F> {
    pub a: Value<F>,
    pub b: Value<F>,
}

impl<F: FieldExt> Circuit<F> for MyCircuit<F> {
    type Config = FiboConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let col_a = meta.advice_column();
        let col_b = meta.advice_column();
        let col_c = meta.advice_column();
        let instance = meta.instance_column();
        FiboChip::configure(meta, [col_a, col_b, col_c], instance)
    }

    fn synthesize(&self, config: Self::Config, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        let chip = FiboChip::construct(config);

        let (prev_a, mut prev_b, mut prev_c) =
            chip.assign_first_row(layouter.namespace(|| "first row"), self.a, self.b)?;

        chip.expose_public(layouter.namespace(|| "private a"), &prev_a, 0)?;
        chip.expose_public(layouter.namespace(|| "private b"), &prev_b, 1)?;

        for _i in 3..10 {
            let c_cell = chip.assign_row(layouter.namespace(|| "next row"), &prev_b, &prev_c)?;
            prev_b = prev_c;
            prev_c = c_cell;
        }

        chip.expose_public(layouter.namespace(|| "out"), &prev_c, 2)?;

        Ok(())
    }
}

fn main() {

    let k = 4;

    let a = Fp::from(1); // F[0]
    let b = Fp::from(1); // F[1]
    let out = Fp::from(55); // F[9]

    let mut public_input = vec![a, b, out];

    let circuit = MyCircuit {
        a: Value::known(a),
        b: Value::known(b)
    };

    let prover = MockProver::run(k, &circuit, vec![public_input.clone()]).unwrap();
    prover.assert_satisfied();

    public_input[2] += Fp::one();
    let _prover = MockProver::run(k, &circuit, vec![public_input]).unwrap();
    // uncomment the following line and the assert will fail
    // _prover.assert_satisfied();
}
