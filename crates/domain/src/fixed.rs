//! Exact integer distribution primitives for later P3 society responses.
//! No floating-point functions and no implicit rounding of independent cells.
use crate::{CohortWeights, ContractError, SHARE_DENOMINATOR};
use crate::world::SupportPpm;

/// Allocate a nonnegative amount to six exclusive population cells by largest
/// remainder. Stable lower cell index wins ties. Outputs conserve all units.
pub fn apportion(units:u64,weights:&CohortWeights)->Result<[u64;6],ContractError>{
    let mut base=[0_u64;6];
    let mut remainder=[0_u128;6];
    let mut accounted=0_u64;
    for (i,weight) in weights.parts_ppm().iter().enumerate(){
        let product=u128::from(units).checked_mul(u128::from(*weight))
            .ok_or(ContractError::ArithmeticOverflow)?;
        let floor=product/u128::from(SHARE_DENOMINATOR);
        base[i]=u64::try_from(floor).map_err(|_|ContractError::ArithmeticOverflow)?;
        accounted=accounted.checked_add(base[i]).ok_or(ContractError::ArithmeticOverflow)?;
        remainder[i]=product%u128::from(SHARE_DENOMINATOR);
    }
    let outstanding=units.checked_sub(accounted).ok_or(ContractError::ArithmeticOverflow)?;
    let mut order=[0_usize,1,2,3,4,5];
    order.sort_by(|&left,&right| remainder[right].cmp(&remainder[left])
        .then_with(||left.cmp(&right)));
    if outstanding>6{return Err(ContractError::ArithmeticOverflow)}
    for i in 0..usize::try_from(outstanding).map_err(|_|ContractError::ArithmeticOverflow)?{
        base[order[i]]=base[order[i]].checked_add(1)
            .ok_or(ContractError::ArithmeticOverflow)?;
    }
    Ok(base)
}

/// Mean support ppm with the fractional residual retained explicitly.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct WeightedSupport {pub floor_ppm:u32,pub residual_ppm:u32}
pub fn weighted_support(weights:&CohortWeights,values:&[SupportPpm;6])
    ->Result<WeightedSupport,ContractError>
{
    let mut sum=0_u128;
    for (weight,value) in weights.parts_ppm().iter().zip(values){
        sum=sum.checked_add(u128::from(*weight)*u128::from(value.value()))
            .ok_or(ContractError::ArithmeticOverflow)?;
    }
    let div=u128::from(SHARE_DENOMINATOR);
    Ok(WeightedSupport{
        floor_ppm:u32::try_from(sum/div).map_err(|_|ContractError::ArithmeticOverflow)?,
        residual_ppm:u32::try_from(sum%div).map_err(|_|ContractError::ArithmeticOverflow)?,
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn ties_are_deterministic_and_conserve_every_unit(){
        let w=CohortWeights::new([200_000,200_000,100_000,150_000,200_000,150_000]).unwrap();
        assert_eq!(apportion(1,&w).unwrap(),[1,0,0,0,0,0]);
        assert_eq!(apportion(10,&w).unwrap().iter().sum::<u64>(),10);
        assert_eq!(apportion(u64::MAX,&w).unwrap().iter().map(|n|u128::from(*n)).sum::<u128>(),
            u128::from(u64::MAX));
    }
    #[test]
    fn weighted_support_tracks_fractional_residual(){
        let w=CohortWeights::new([200_000,200_000,100_000,150_000,200_000,150_000]).unwrap();
        let v=[SupportPpm::new(500_000).unwrap();6];
        assert_eq!(weighted_support(&w,&v).unwrap(),WeightedSupport{
            floor_ppm:500_000,residual_ppm:0});
        let mut changed=v;
        changed[2]=SupportPpm::new(500_001).unwrap();
        assert_eq!(weighted_support(&w,&changed).unwrap(),WeightedSupport{
            floor_ppm:500_000,residual_ppm:100_000});
    }
}
