//! Who the voters are (decision 0015).
//!
//! The pilot began with six groups, urban or rural by low, middle or high income, and the number six was typed into the engine.
//! That does not stretch to ideological, political, social or economic groups: these overlap and sit in layers, because one voter
//! is urban *and* low income *and* a union member *and* progressive. So the model here has three levels:
//!
//! * **Attributes** (geography, income, outlook, ...), each with a few **levels**.
//! * **Cells**: every combination of one level of each attribute, with a weight, the share of the voters in it. The cells
//!   partition the voters exactly, so the election counts each voter once. Support is tracked per cell.
//! * **Groups**: a named rule over the cells (a *selector*: "rural", "rural and low income", "progressive or traditional but not
//!   high income"). Groups may overlap and nest freely. A group's weight and support are those of its cells, so they are always
//!   consistent with the cells and the election.
//!
//! Interests are expressed by a **kernel**: a base response per kind of work times one multiplier per attribute level, so
//! adding an attribute means adding one table of multipliers, never changing a formula. For the pilot's two attributes this is
//! exactly the response the engine has always had (a test checks every cell, plan and magnitude).
//!
//! Everything here is integer arithmetic, so a replay is exact on every platform.

use crate::{ContractError, SHARE_DENOMINATOR};

/// The most cells a population may have: enough for a rich cross of attributes, small enough to keep every screen and replay cheap.
pub const MAX_CELLS: usize = 64;
/// The most attributes a population may cross.
pub const MAX_ATTRIBUTES: usize = 6;
/// The most groups (cells included) a population may name.
pub const MAX_GROUPS: usize = 128;
/// An attribute or level identifier: lower-case letters and digits only, so that a cell's identifier (its levels joined by an
/// underscore) can only be read one way.
fn plain_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}
/// A group identifier: lower-case letters, digits and underscores, as used in the scenario data and the API.
fn group_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('_')
        && !s.ends_with('_')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
/// Channels of the response kernel: staffing, repairs and access benefit, then the cost the work puts on voters.
pub const CHANNELS: usize = 4;

/// One level of an attribute (for example `rural` of geography).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level {
    pub id: String,
    pub label: String,
}
/// A way voters differ, with the levels they can have (geography, income, outlook, union membership, ...).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub id: String,
    pub levels: Vec<Level>,
}
/// A rule that picks cells. `Is` names an attribute and one of its levels by position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selector {
    Everyone,
    Is { attribute: usize, level: usize },
    All(Vec<Selector>),
    Any(Vec<Selector>),
    Not(Box<Selector>),
}
impl Selector {
    fn holds(&self, cell: &[usize]) -> bool {
        match self {
            Self::Everyone => true,
            Self::Is { attribute, level } => cell.get(*attribute) == Some(level),
            Self::All(all) => all.iter().all(|s| s.holds(cell)),
            Self::Any(any) => any.iter().any(|s| s.holds(cell)),
            Self::Not(inner) => !inner.holds(cell),
        }
    }
    fn valid(&self, attributes: &[Attribute], depth: usize) -> bool {
        depth <= 6
            && match self {
                Self::Everyone => true,
                Self::Is { attribute, level } => attributes
                    .get(*attribute)
                    .is_some_and(|a| *level < a.levels.len()),
                Self::All(v) | Self::Any(v) => {
                    !v.is_empty() && v.iter().all(|s| s.valid(attributes, depth + 1))
                }
                Self::Not(inner) => inner.valid(attributes, depth + 1),
            }
    }
}
/// A named, labelled selection of voters. Groups overlap and nest freely.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group {
    pub id: String,
    pub label: String,
    pub selector: Selector,
}
/// How a cell responds to delivered work: `base_ppm` per unit of staffing, repairs, access and (last) the cost the work puts on the
/// voters, times, for each attribute, the percentage for the cell's level of it. `percent[attribute][level][channel]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Kernel {
    pub base_ppm: [i128; CHANNELS],
    pub percent: Vec<Vec<[i128; CHANNELS]>>,
}

/// A population: the attributes, the weight of each cell, the groups and the kernel. Built once, checked, and read-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Population {
    attributes: Vec<Attribute>,
    /// For each cell, the level of each attribute, the first attribute varying slowest (so with geography then income the cells
    /// run urban low, urban middle, urban high, rural low, ...).
    cells: Vec<Vec<usize>>,
    weights_ppm: Vec<u32>,
    kernel: Kernel,
    /// Every cell is a group, in cell order; then the declared groups.
    group_ids: Vec<String>,
    group_labels: Vec<String>,
    group_members: Vec<Vec<usize>>,
}

impl Population {
    /// A population from its parts. Fails unless the structure is sound: bounded sizes, one weight per cell summing to a million
    /// ppm, a kernel with a table for every attribute level, identifiers that are well formed and unique, selectors that name
    /// real levels, and no group that selects no cell.
    pub fn new(
        attributes: Vec<Attribute>,
        weights_ppm: Vec<u32>,
        declared: Vec<Group>,
        kernel: Kernel,
    ) -> Result<Self, ContractError> {
        let sound = !attributes.is_empty()
            && attributes.len() <= MAX_ATTRIBUTES
            && attributes
                .iter()
                .all(|a| !a.levels.is_empty() && a.levels.len() <= MAX_CELLS);
        if !sound {
            return Err(ContractError::InvalidCondition);
        }
        for a in &attributes {
            if !plain_id(&a.id) {
                return Err(ContractError::InvalidCondition);
            }
            for l in &a.levels {
                if !plain_id(&l.id) || l.label.is_empty() {
                    return Err(ContractError::InvalidCondition);
                }
            }
        }
        let count = attributes
            .iter()
            .try_fold(1_usize, |n, a| n.checked_mul(a.levels.len()))
            .filter(|n| *n <= MAX_CELLS)
            .ok_or(ContractError::InvalidCondition)?;
        if weights_ppm.len() != count
            || weights_ppm.iter().map(|w| u64::from(*w)).sum::<u64>()
                != u64::from(SHARE_DENOMINATOR)
        {
            return Err(ContractError::InvalidPopulationWeights);
        }
        if kernel.percent.len() != attributes.len()
            || kernel
                .percent
                .iter()
                .zip(&attributes)
                .any(|(table, a)| table.len() != a.levels.len())
            || kernel.base_ppm.iter().any(|b| *b < 0)
            || kernel.percent.iter().flatten().flatten().any(|p| *p < 0)
        {
            return Err(ContractError::InvalidCondition);
        }
        // Every combination of levels, the first attribute varying slowest.
        let mut cells: Vec<Vec<usize>> = vec![vec![]];
        for a in &attributes {
            cells = cells
                .into_iter()
                .flat_map(|prefix| {
                    (0..a.levels.len()).map(move |level| {
                        let mut next = prefix.clone();
                        next.push(level);
                        next
                    })
                })
                .collect();
        }
        let mut group_ids = vec![];
        let mut group_labels = vec![];
        let mut group_members = vec![];
        for levels in &cells {
            let id = levels
                .iter()
                .zip(&attributes)
                .map(|(l, a)| a.levels[*l].id.as_str())
                .collect::<Vec<_>>()
                .join("_");
            let label = levels
                .iter()
                .zip(&attributes)
                .map(|(l, a)| a.levels[*l].label.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            group_members.push(vec![group_ids.len()]);
            group_ids.push(id);
            group_labels.push(label);
        }
        if count + declared.len() > MAX_GROUPS {
            return Err(ContractError::InvalidCondition);
        }
        for g in declared {
            if !group_id(&g.id) || g.label.is_empty() || !g.selector.valid(&attributes, 0) {
                return Err(ContractError::InvalidCondition);
            }
            let members: Vec<usize> = cells
                .iter()
                .enumerate()
                .filter(|(_, levels)| g.selector.holds(levels))
                .map(|(i, _)| i)
                .collect();
            if members.is_empty() {
                return Err(ContractError::InvalidCondition);
            }
            group_ids.push(g.id);
            group_labels.push(g.label);
            group_members.push(members);
        }
        let mut sorted = group_ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != group_ids.len() {
            return Err(ContractError::InvalidCondition);
        }
        Ok(Self {
            attributes,
            cells,
            weights_ppm,
            kernel,
            group_ids,
            group_labels,
            group_members,
        })
    }

    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }
    /// How many cells the voters are divided into. Everything that is tracked per cell has this length.
    pub fn cells(&self) -> usize {
        self.cells.len()
    }
    /// Each cell's share of the voters, in ppm; sums to a million.
    pub fn weights_ppm(&self) -> &[u32] {
        &self.weights_ppm
    }
    /// The level of each attribute that a cell has.
    pub fn cell_levels(&self, cell: usize) -> Option<&[usize]> {
        self.cells.get(cell).map(Vec::as_slice)
    }
    /// A cell's identifier, its levels joined by an underscore (`urban_low`).
    pub fn cell_id(&self, cell: usize) -> Option<&str> {
        (cell < self.cells.len()).then(|| self.group_ids[cell].as_str())
    }
    /// Every group, the cells first (in cell order) and then the declared groups.
    pub fn group_count(&self) -> usize {
        self.group_ids.len()
    }
    pub fn group_id(&self, group: usize) -> Option<&str> {
        self.group_ids.get(group).map(String::as_str)
    }
    pub fn group_label(&self, group: usize) -> Option<&str> {
        self.group_labels.get(group).map(String::as_str)
    }
    /// The cells a group selects, in cell order.
    pub fn members(&self, group: usize) -> Option<&[usize]> {
        self.group_members.get(group).map(Vec::as_slice)
    }
    pub fn group_index(&self, id: &str) -> Option<usize> {
        self.group_ids.iter().position(|g| g == id)
    }
    /// A group's share of the voters, in ppm: the weights of its cells. Overlapping groups share voters, so these need not add up.
    pub fn group_weight_ppm(&self, group: usize) -> Option<u32> {
        self.members(group)
            .map(|m| m.iter().map(|c| self.weights_ppm[*c]).sum())
    }
    /// A group's support given every cell's support, in ppm: the cells' average weighted by their share of the voters,
    /// truncated. A group with no voters has none.
    pub fn group_support_ppm(&self, group: usize, cell_support_ppm: &[u32]) -> Option<u32> {
        if cell_support_ppm.len() != self.cells.len() {
            return None;
        }
        let members = self.members(group)?;
        let weight: u64 = members
            .iter()
            .map(|c| u64::from(self.weights_ppm[*c]))
            .sum();
        if weight == 0 {
            return Some(0);
        }
        let total: u128 = members
            .iter()
            .map(|c| u128::from(self.weights_ppm[*c]) * u128::from(cell_support_ppm[*c]))
            .sum();
        u32::try_from(total / u128::from(weight)).ok()
    }
    /// The whole electorate's support given every cell's support, in ppm, truncated.
    pub fn overall_support_ppm(&self, cell_support_ppm: &[u32]) -> Option<u32> {
        if cell_support_ppm.len() != self.cells.len() {
            return None;
        }
        let total: u128 = self
            .weights_ppm
            .iter()
            .zip(cell_support_ppm)
            .map(|(w, s)| u128::from(*w) * u128::from(*s))
            .sum();
        u32::try_from(total / u128::from(SHARE_DENOMINATOR)).ok()
    }
    /// Spread entries named by group onto the cells: each cell gets the sum of the entries of the groups it belongs to, never more
    /// than `cap`. `[("rural", 40)]` is 40 in every rural cell; `[("rural", 40), ("low_income", 30)]` is 70 in the cells that are
    /// both. A name that is not a group is an error, never a silent zero.
    pub fn spread(&self, entries: &[(&str, u32)], cap: u32) -> Result<Vec<u32>, ContractError> {
        let mut out = vec![0_u32; self.cells.len()];
        for (name, value) in entries {
            let group = self
                .group_index(name)
                .ok_or(ContractError::InvalidCondition)?;
            for cell in &self.group_members[group] {
                out[*cell] = out[*cell].saturating_add(*value).min(cap);
            }
        }
        Ok(out)
    }
    /// What delivering `delivered` units (staffing, repairs, access) adds to one cell's support in one quarter, in ppm, before the
    /// clip at `clip`. Each channel is the base response times the cell's multiplier for each attribute, in percent, divided once at
    /// the end; the cost channel is charged on every unit; the sum is scaled by `magnitude_percent`. A negative number is a cell
    /// that pays more than it gains.
    pub fn response_ppm(
        &self,
        cell: usize,
        delivered: [u16; 3],
        magnitude_percent: u16,
        clip: i64,
    ) -> Result<i64, ContractError> {
        let levels = self
            .cells
            .get(cell)
            .ok_or(ContractError::InvalidCondition)?;
        let divisor = 100_i128
            .pow(u32::try_from(levels.len()).map_err(|_| ContractError::ArithmeticOverflow)?);
        let mut terms = [0_i128; CHANNELS];
        for (channel, term) in terms.iter_mut().enumerate() {
            let mut v = self.kernel.base_ppm[channel];
            for (attribute, level) in levels.iter().enumerate() {
                v = v
                    .checked_mul(self.kernel.percent[attribute][*level][channel])
                    .ok_or(ContractError::ArithmeticOverflow)?;
            }
            *term = v / divisor;
        }
        let spent: i128 = delivered.iter().map(|v| i128::from(*v)).sum();
        let result = terms[0] * i128::from(delivered[0])
            + terms[1] * i128::from(delivered[1])
            + terms[2] * i128::from(delivered[2])
            - terms[3] * spent;
        let scaled = result
            .checked_mul(i128::from(magnitude_percent))
            .ok_or(ContractError::ArithmeticOverflow)?
            / 100;
        let clip = i128::from(clip);
        i64::try_from(scaled.clamp(-clip, clip)).map_err(|_| ContractError::ArithmeticOverflow)
    }

    /// The pilot's six groups: geography (urban, rural) by income (low, middle, high), with these shares of the voters, and the
    /// groups a reader would also name: urban, rural, and each income.
    pub fn pilot_with(weights_ppm: [u32; 6]) -> Result<Self, ContractError> {
        let level = |id: &str, label: &str| Level {
            id: id.into(),
            label: label.into(),
        };
        let attributes = vec![
            Attribute {
                id: "geography".into(),
                levels: vec![level("urban", "Urban"), level("rural", "Rural")],
            },
            Attribute {
                id: "income".into(),
                levels: vec![
                    level("low", "low income"),
                    level("middle", "middle income"),
                    level("high", "high income"),
                ],
            },
        ];
        let group = |id: &str, label: &str, attribute: usize, level: usize| Group {
            id: id.into(),
            label: label.into(),
            selector: Selector::Is { attribute, level },
        };
        let declared = vec![
            group("urban", "All urban voters", 0, 0),
            group("rural", "All rural voters", 0, 1),
            group("low_income", "All lower-income voters", 1, 0),
            group("middle_income", "All middle-income voters", 1, 1),
            group("high_income", "All higher-income voters", 1, 2),
            Group {
                id: "everyone".into(),
                label: "All voters".into(),
                selector: Selector::Everyone,
            },
        ];
        let kernel = Kernel {
            base_ppm: crate::reducer::RESPONSE_BASE_PPM,
            percent: vec![
                crate::reducer::GEOGRAPHY_PERCENT.to_vec(),
                crate::reducer::INCOME_PERCENT.to_vec(),
            ],
        };
        Self::new(attributes, weights_ppm.to_vec(), declared, kernel)
    }
    /// The pilot's authored baseline shares.
    pub fn pilot() -> Result<Self, ContractError> {
        Self::pilot_with([260_000, 320_000, 220_000, 70_000, 80_000, 50_000])
    }
    /// A twelve-cell population that adds a third attribute, outlook (progressive or traditional), to the pilot's two. It exists
    /// to show, and to test, that nothing in the engine is tied to six groups: groups overlap (rural, traditional, rural and
    /// traditional), the kernel gains one table, and the election still counts each voter once. It is a fixture, not a claim
    /// about any real electorate.
    pub fn demo_twelve_cells() -> Result<Self, ContractError> {
        let pilot = Self::pilot()?;
        let level = |id: &str, label: &str| Level {
            id: id.into(),
            label: label.into(),
        };
        let mut attributes = pilot.attributes.clone();
        attributes.push(Attribute {
            id: "outlook".into(),
            levels: vec![
                level("progressive", "progressive"),
                level("traditional", "traditional"),
            ],
        });
        // Each pilot cell is split between the two outlooks: progressive voters are a larger share where incomes are higher.
        let progressive_percent = [40_u32, 55, 65, 40, 55, 65];
        let mut weights = vec![];
        for (cell, w) in pilot.weights_ppm.iter().enumerate() {
            let progressive = w * progressive_percent[cell] / 100;
            weights.push(progressive);
            weights.push(w - progressive);
        }
        let is = |attribute: usize, level: usize| Selector::Is { attribute, level };
        let declared = vec![
            Group {
                id: "everyone".into(),
                label: "All voters".into(),
                selector: Selector::Everyone,
            },
            Group {
                id: "urban".into(),
                label: "All urban voters".into(),
                selector: is(0, 0),
            },
            Group {
                id: "rural".into(),
                label: "All rural voters".into(),
                selector: is(0, 1),
            },
            Group {
                id: "low_income".into(),
                label: "All lower-income voters".into(),
                selector: is(1, 0),
            },
            Group {
                id: "middle_income".into(),
                label: "All middle-income voters".into(),
                selector: is(1, 1),
            },
            Group {
                id: "high_income".into(),
                label: "All higher-income voters".into(),
                selector: is(1, 2),
            },
            Group {
                id: "progressive".into(),
                label: "All progressive voters".into(),
                selector: is(2, 0),
            },
            Group {
                id: "traditional".into(),
                label: "All traditional voters".into(),
                selector: is(2, 1),
            },
            Group {
                id: "rural_traditional".into(),
                label: "Rural traditional voters".into(),
                selector: Selector::All(vec![is(0, 1), is(2, 1)]),
            },
            Group {
                id: "left_behind".into(),
                label: "Lower-income voters outside the progressive cities".into(),
                selector: Selector::All(vec![
                    is(1, 0),
                    Selector::Not(Box::new(Selector::All(vec![is(0, 0), is(2, 0)]))),
                ]),
            },
        ];
        // The pilot's six cells are still groups here, each now spanning both outlooks: the news deck is written in terms of them.
        let mut declared = declared;
        for (geography, place) in ["urban", "rural"].into_iter().enumerate() {
            for (income, band) in ["low", "middle", "high"].into_iter().enumerate() {
                declared.push(Group {
                    id: format!("{place}_{band}"),
                    label: format!("{place} {band}-income voters"),
                    selector: Selector::All(vec![is(0, geography), is(1, income)]),
                });
            }
        }
        let mut percent = pilot.kernel.percent.clone();
        // Progressive voters value staffing and access a little more and mind the cost less; traditional voters the reverse.
        percent.push(vec![[110, 90, 110, 80], [90, 110, 90, 125]]);
        Self::new(
            attributes,
            weights,
            declared,
            Kernel {
                base_ppm: pilot.kernel.base_ppm,
                percent,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reducer::{RESPONSE_CLIP_PPM, cohort_response_scaled};

    #[test]
    fn the_pilot_kernel_is_exactly_the_engines_response_for_every_cell_plan_and_magnitude() {
        let pilot = Population::pilot().unwrap();
        assert_eq!(pilot.cells(), 6);
        let mut checked = 0;
        for cell in 0..6 {
            for magnitude in [100_u16, 80, 125, 1] {
                for plan in [
                    [0_u16, 0, 0],
                    [1, 0, 0],
                    [0, 1, 0],
                    [0, 0, 1],
                    [12, 10, 4],
                    [3, 7, 2],
                    [50, 40, 10],
                    [200, 0, 0],
                    [0, 0, 300],
                    [65_535, 65_535, 65_535],
                ] {
                    let engine = cohort_response_scaled(cell, plan, magnitude).unwrap();
                    let model = pilot
                        .response_ppm(cell, plan, magnitude, RESPONSE_CLIP_PPM)
                        .unwrap();
                    assert_eq!(
                        model, engine,
                        "cell {cell}, magnitude {magnitude}, plan {plan:?}"
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 6 * 4 * 10);
    }

    #[test]
    fn cells_are_every_combination_with_the_first_attribute_slowest_and_each_is_a_group() {
        let pilot = Population::pilot().unwrap();
        let ids: Vec<_> = (0..6).map(|c| pilot.cell_id(c).unwrap()).collect();
        assert_eq!(
            ids,
            [
                "urban_low",
                "urban_middle",
                "urban_high",
                "rural_low",
                "rural_middle",
                "rural_high"
            ]
        );
        assert_eq!(pilot.group_label(2), Some("Urban, high income"));
        assert_eq!(pilot.group_label(3), Some("Rural, low income"));
        assert_eq!(pilot.cell_levels(4), Some(&[1_usize, 1][..]));
        // Cells first, then the declared groups.
        assert_eq!(pilot.group_count(), 6 + 6);
        assert_eq!(pilot.group_index("rural_high"), Some(5));
        assert_eq!(pilot.group_index("high_income"), Some(6 + 4));
        assert_eq!(
            pilot.members(pilot.group_index("urban").unwrap()),
            Some(&[0_usize, 1, 2][..])
        );
        assert_eq!(
            pilot.members(pilot.group_index("high_income").unwrap()),
            Some(&[2_usize, 5][..])
        );
        assert_eq!(
            pilot.members(3),
            Some(&[3_usize][..]),
            "a cell selects itself"
        );
    }

    #[test]
    fn a_group_weighs_and_supports_what_its_cells_do_and_the_election_counts_every_voter_once() {
        let pilot = Population::pilot().unwrap();
        let support = [600_000, 620_000, 480_000, 610_000, 640_000, 500_000];
        // The same arithmetic the election has always done: weighted by share, truncated.
        assert_eq!(pilot.overall_support_ppm(&support), Some(578_900));
        let urban = pilot.group_index("urban").unwrap();
        let rural = pilot.group_index("rural").unwrap();
        // Urban and rural partition the voters, so their weights add up exactly.
        assert_eq!(
            pilot.group_weight_ppm(urban).unwrap() + pilot.group_weight_ppm(rural).unwrap(),
            1_000_000
        );
        assert_eq!(pilot.group_weight_ppm(urban), Some(800_000));
        // Urban support: (260000*600000 + 320000*620000 + 220000*480000) / 800000 = 575,000.
        assert_eq!(pilot.group_support_ppm(urban, &support), Some(575_000));
        // A cell is its own average; everyone is the overall.
        assert_eq!(pilot.group_support_ppm(3, &support), Some(610_000));
        // Wrong length: refused, never guessed.
        assert_eq!(pilot.group_support_ppm(urban, &support[..5]), None);
    }

    #[test]
    fn a_population_is_refused_unless_it_is_sound() {
        let pilot = Population::pilot().unwrap();
        let (attributes, kernel) = (pilot.attributes.clone(), pilot.kernel.clone());
        let ok = |weights: Vec<u32>, declared: Vec<Group>, kernel: Kernel| {
            Population::new(attributes.clone(), weights, declared, kernel)
        };
        let weights = pilot.weights_ppm.clone();
        assert!(ok(weights.clone(), vec![], kernel.clone()).is_ok());
        // Weights: one per cell, summing to a million.
        assert!(ok(weights[..5].to_vec(), vec![], kernel.clone()).is_err());
        let mut off = weights.clone();
        off[0] += 1;
        assert!(ok(off, vec![], kernel.clone()).is_err());
        // The kernel needs a table for every level of every attribute.
        let mut short = kernel.clone();
        short.percent[1].pop();
        assert!(ok(weights.clone(), vec![], short).is_err());
        let mut missing = kernel.clone();
        missing.percent.pop();
        assert!(ok(weights.clone(), vec![], missing).is_err());
        // Groups: well-formed, unique, selecting something that exists.
        let group = |id: &str, selector: Selector| Group {
            id: id.into(),
            label: "A group".into(),
            selector,
        };
        let rural = Selector::Is {
            attribute: 0,
            level: 1,
        };
        assert!(
            ok(
                weights.clone(),
                vec![group("rural", rural.clone())],
                kernel.clone()
            )
            .is_ok()
        );
        assert!(
            ok(
                weights.clone(),
                vec![group("rural", rural.clone()), group("rural", rural.clone())],
                kernel.clone()
            )
            .is_err(),
            "duplicate"
        );
        assert!(
            ok(
                weights.clone(),
                vec![group("urban_low", rural.clone())],
                kernel.clone()
            )
            .is_err(),
            "a cell's own id"
        );
        assert!(
            ok(
                weights.clone(),
                vec![group("bad id", rural.clone())],
                kernel.clone()
            )
            .is_err(),
            "not an identifier"
        );
        assert!(
            ok(
                weights.clone(),
                vec![group(
                    "nobody",
                    Selector::Is {
                        attribute: 0,
                        level: 9
                    }
                )],
                kernel.clone()
            )
            .is_err(),
            "no such level"
        );
        assert!(
            ok(
                weights.clone(),
                vec![group(
                    "nobody",
                    Selector::Is {
                        attribute: 7,
                        level: 0
                    }
                )],
                kernel.clone()
            )
            .is_err(),
            "no such attribute"
        );
        assert!(
            ok(
                weights.clone(),
                vec![group("none", Selector::Not(Box::new(Selector::Everyone)))],
                kernel.clone()
            )
            .is_err(),
            "selects nothing"
        );
        assert!(
            ok(
                weights.clone(),
                vec![group("empty_all", Selector::All(vec![]))],
                kernel.clone()
            )
            .is_err()
        );
        // Levels are plain words, so a cell's identifier can only be read one way.
        let mut underscored = attributes.clone();
        underscored[0].levels[0].id = "ur_ban".into();
        assert!(Population::new(underscored, weights.clone(), vec![], kernel.clone()).is_err());
        // Entries by group name are refused when the name is not a group.
        assert!(pilot.spread(&[("nowhere", 10)], 100).is_err());
    }

    #[test]
    fn entries_by_group_name_spread_onto_the_cells_they_select() {
        let pilot = Population::pilot().unwrap();
        // A single cell by its own id: exactly the six-number vector the news decks have always used.
        assert_eq!(
            pilot
                .spread(&[("urban_low", 50), ("rural_high", 25)], 100)
                .unwrap(),
            [50, 0, 0, 0, 0, 25]
        );
        // A wider group reaches every cell in it.
        assert_eq!(
            pilot.spread(&[("rural", 70)], 100).unwrap(),
            [0, 0, 0, 70, 70, 70]
        );
        // Overlapping entries add where they overlap, never past the cap.
        assert_eq!(
            pilot
                .spread(&[("rural", 70), ("high_income", 50)], 100)
                .unwrap(),
            [0, 0, 50, 70, 70, 100]
        );
        assert_eq!(
            pilot
                .spread(&[("rural", 70), ("high_income", 50)], 90)
                .unwrap()[5],
            90
        );
        assert_eq!(pilot.spread(&[], 100).unwrap(), [0; 6]);
        assert_eq!(
            pilot.spread(&[("everyone", 15_000)], 40_000).unwrap(),
            [15_000; 6]
        );
    }

    #[test]
    fn a_twelve_cell_population_with_overlapping_groups_works_with_nothing_changed_but_data() {
        let twelve = Population::demo_twelve_cells().unwrap();
        assert_eq!(twelve.cells(), 12);
        assert_eq!(
            twelve
                .weights_ppm()
                .iter()
                .map(|w| u64::from(*w))
                .sum::<u64>(),
            1_000_000
        );
        assert_eq!(twelve.cell_id(0), Some("urban_low_progressive"));
        assert_eq!(
            twelve.group_label(11),
            Some("Rural, high income, traditional")
        );
        // Groups overlap and nest: rural traditional is inside both rural and traditional.
        let (rural, traditional, both) = (
            twelve.group_index("rural").unwrap(),
            twelve.group_index("traditional").unwrap(),
            twelve.group_index("rural_traditional").unwrap(),
        );
        let m = |g| twelve.members(g).unwrap().to_vec();
        assert_eq!(m(rural).len(), 6);
        assert_eq!(m(traditional).len(), 6);
        assert_eq!(m(both), [7, 9, 11].map(|c| c));
        assert!(
            m(both)
                .iter()
                .all(|c| m(rural).contains(c) && m(traditional).contains(c))
        );
        // Overlapping groups share voters: their weights add up to more than the voters in either of them, by exactly the voters
        // they share...
        let (w_rural, w_trad, w_both) = (
            twelve.group_weight_ppm(rural).unwrap(),
            twelve.group_weight_ppm(traditional).unwrap(),
            twelve.group_weight_ppm(both).unwrap(),
        );
        let mut union: Vec<usize> = m(rural);
        union.extend(m(traditional));
        union.sort_unstable();
        union.dedup();
        let w_union: u32 = union.iter().map(|c| twelve.weights_ppm()[*c]).sum();
        assert_eq!(w_rural + w_trad - w_both, w_union);
        assert!(w_rural + w_trad > w_union && w_both > 0);
        // ...while the election counts each voter once, by cell.
        let support: Vec<u32> = (0..12).map(|c| 480_000 + 10_000 * c as u32).collect();
        let overall = twelve.overall_support_ppm(&support).unwrap();
        let by_cell: u128 = twelve
            .weights_ppm()
            .iter()
            .zip(&support)
            .map(|(w, s)| u128::from(*w) * u128::from(*s))
            .sum();
        assert_eq!(u128::from(overall), by_cell / 1_000_000);
        // A group of "lower-income voters outside the progressive cities" is a real selection through And, Not and nesting.
        let behind = twelve
            .members(twelve.group_index("left_behind").unwrap())
            .unwrap()
            .to_vec();
        assert_eq!(behind, [1, 6, 7]);
        // The third attribute's multipliers change the response with no change to any formula: same work, different cells.
        let plan = [12, 10, 4];
        let progressive_urban_low = twelve
            .response_ppm(0, plan, 100, RESPONSE_CLIP_PPM)
            .unwrap();
        let traditional_urban_low = twelve
            .response_ppm(1, plan, 100, RESPONSE_CLIP_PPM)
            .unwrap();
        assert_ne!(progressive_urban_low, traditional_urban_low);
        assert!(
            progressive_urban_low > traditional_urban_low,
            "progressive voters value the work more and mind its cost less"
        );
        // Spreading by an overlapping group touches exactly its cells.
        let spread = twelve
            .spread(&[("rural_traditional", 60), ("progressive", 10)], 100)
            .unwrap();
        assert_eq!(spread[7], 60);
        assert_eq!(spread[0], 10);
        assert_eq!(spread[1], 0);
        assert_eq!(spread.len(), 12);
    }
}
