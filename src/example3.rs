use halo2_proofs::{arithmetic::FieldExt, circuit::*, plonk::*, poly::Rotation};
use std::marker::PhantomData;

#[derive(Debug, Clone)]
struct ACell<F: FieldExt>(AssignedCell<F, F>);

#[derive(Debug, Clone)]
struct FiboConfig {
    advice: [Column<Advice>; 2],
    selector: Selector,
    instance: Column<Instance>,
}

#[derive(Debug, Clone)]
struct FiboChip<F: FieldExt> {
    config: FiboConfig,
    _marker: PhantomData<F>,
}

impl<F: FieldExt> FiboChip<F> {
    pub fn construct(config: FiboConfig) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }

    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: [Column<Advice>; 2],
        instance: Column<Instance>,
    ) -> FiboConfig {
        let col_a = advice[0];
        let col_b = advice[1];
        let selector = meta.selector();

        // copy constraint を追加するために enable_equality で有効化する必要がある
        meta.enable_equality(col_a);
        meta.enable_equality(col_b);
        meta.enable_equality(instance);

        meta.create_gate("add1", |meta| {
            //
            // col_a | col_b | selector
            //   a      b        s
            //   c      d
            let s = meta.query_selector(selector);
            let a = meta.query_advice(col_a, Rotation::cur());
            let b = meta.query_advice(col_b, Rotation::cur());
            let c = meta.query_advice(col_a, Rotation::next());
            let d = meta.query_advice(col_b, Rotation::next());
            vec![s.clone() * (a.clone() + b.clone() - c.clone()),  s * (b + c - d)]
        });

        FiboConfig {
            advice: [col_a, col_b],
            selector,
            instance,
        }
    }

    pub fn assign(
        &self,
        mut layouter: impl Layouter<F>,
        nrows: usize,
    ) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "entire fibonacci table",
            |mut region| {

                self.config.selector.enable(&mut region, 0)?;

                let mut a_cell = region.assign_advice_from_instance(
                    || "1",
                    self.config.instance,
                    0,
                    self.config.advice[0],
                    0,
                )?;

                let mut b_cell = region.assign_advice_from_instance(
                    || "1",
                    self.config.instance,
                    0,
                    self.config.advice[1],
                    0,
                )?;

                self.config.selector.enable(&mut region, 1)?;

                a_cell = region.assign_advice(
                    || "advice",
                    self.config.advice[0],
                    1,
                    || a_cell.value().copied() + b_cell.value().copied(),
                )?;

                b_cell = region.assign_advice(
                    || "advice",
                    self.config.advice[1],
                    1,
                    || a_cell.value().copied() + b_cell.value().copied(),
                )?;

                for row in 2.. nrows {
                    if row < nrows-1 {
                        self.config.selector.enable(&mut region, row)?;
                    }

                    a_cell = region.assign_advice(
                        || "advice",
                        self.config.advice[0],
                        row,
                        || a_cell.value().copied() + b_cell.value().copied(),
                    )?;

                    b_cell = region.assign_advice(
                        || "advice",
                        self.config.advice[1],
                        row,
                        || a_cell.value().copied() + b_cell.value().copied(),
                    )?;

                }       

                Ok(b_cell)
            },
        )
    }

    pub fn expose_public(
        &self,
        mut layouter: impl Layouter<F>,
        cell: AssignedCell<F, F>,
        row: usize,
    ) -> Result<(), Error> {
        layouter.constrain_instance(cell.cell(), self.config.instance, row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::{dev::MockProver, pasta::Fp};

    #[derive(Default)]
    struct MyCircuit<F>(PhantomData<F>);

    impl<F: FieldExt> Circuit<F> for MyCircuit<F> {
        type Config = FiboConfig;
        type FloorPlanner = SimpleFloorPlanner;

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let col_a = meta.advice_column();
            let col_b = meta.advice_column();
            let instance = meta.instance_column();
            FiboChip::configure(meta, [col_a, col_b], instance)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            let chip = FiboChip::construct(config);

            let out_cell = chip.assign(layouter.namespace(|| "entire table"), 5)?;

            chip.expose_public(layouter.namespace(|| "out"), out_cell, 2)?;

            Ok(())
        }
    }

    #[test]
    fn test_example3() {
        let k = 4;

        let a = Fp::from(1); // F[0]
        let b = Fp::from(1); // F[1]
        let out = Fp::from(55); // F[9]

        let circuit = MyCircuit(PhantomData);

        let mut public_input = vec![a, b, out];

        let prover = MockProver::run(k, &circuit, vec![public_input.clone()]).unwrap();
        prover.assert_satisfied();

        // public_input[2] += Fp::one();
        // let _prover = MockProver::run(k, &circuit, vec![public_input]).unwrap();
        // uncomment the following line and the assert will fail
        // _prover.assert_satisfied();
    }

    #[cfg(feature = "dev-graph")]
    #[test]
    fn plot_fibo3() {
        use plotters::prelude::*;
        let root = BitMapBackend::new("fib-3-layout.png", (1024, 3096)).into_drawing_area();
        root.fill(&WHITE).unwrap();
        let root = root.titled("Fib 3 Layout", ("sans-serif", 60)).unwrap();

        let circuit = MyCircuit::<Fp>(PhantomData);
        halo2_proofs::dev::CircuitLayout::default()
            .render(4, &circuit, &root)
            .unwrap();
    }
}