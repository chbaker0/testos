#[allow(dead_code)]
static TEST: usize = 0;
#[allow(dead_code)]
static mut TEST_MUT: usize = 0;

pub fn test() {
    assert!(TEST == 0);
    assert!(unsafe { TEST_MUT } == 0);
}
