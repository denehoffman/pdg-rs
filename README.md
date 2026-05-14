# pdg-rs

This crate enables programmatic access to the [Particle Data Group](https://pdg.lbl.gov)'s database of particle physics measurements. This project is independently developed and not affiliated with the PDG. Please use the citation at the end of this README as you would if you were using the PDG website!

## Installation
```bash
cargo add pdg-rs
```
> [!NOTE]
> This crate has not yet been published, so this command will not actually function yet.

## Usage

### Get information about a specific particle:
```rust
use pdg_rs::{Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open()?;
    let pi_plus = pdg.particle("pi+")?.unwrap();
    // alternatively `pdg.pdgid(211)?.unwrap()`
    println!("{pi_plus}");
    let m_pi_plus = pi_plus.mass()?.unwrap();
    println!("{m_pi_plus}");
    Ok(())
}

// pi+ (S008, Meson, Particle, charge +1), MCID 211, I=1, G=-, J=0, P=-
// 139.57039+-0.00018 MeV
```
Most queries return `Option`s because we can't guarantee the search term will yield results (for instance, the "p-" doesn't exist, so asking for it should return `None`). The `PdgError` type is mostly reserved for errors encountered while parsing data from the sqlite database. Particle information is either directly available (charge, quantum numbers) or queryable (mass, lifetime). Most of these methods will produce unique data types (like a `Mass` struct) which contain additional information:

```rust
let mass_texts = pdg.texts_for(&m_pi_plus.pdg_id)?;
for text in mass_texts {
    if let Some(text) = text.text {
        println!("{}", text);
    }
}
let mass_measurements = pdg.measurements_for(&m_pi_plus.pdg_id)?;
for measurement in mass_measurements {
    println!(
        "{:<20} {:<50}: {}",
        measurement.reference.document_id,
        measurement.reference.doi.unwrap_or_default(),
        measurement
            .values
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
}
```
This will output:
```text
The most accurate charged pion mass measurements are based upon x-ray wavelength measurements for transitions in pi--mesonic atoms.  The observed line is the blend of three
 components, corresponding to different K-shell occupancies.  JECKELMANN 1994 revisits the occupancy question, with the conclusion that two sets of occupancy ratios, result
ing in two different pion masses (Solutions A and B), are equally probable. We choose the higher Solution B since only this solution is consistent with a positive mass-squa
red for the muon neutrino, given the precise muon momentum measurements now available (DAUM 1991, ASSAMAGAN 1994, and ASSAMAGAN 1996) for the decay of pions at rest.  Earli
er mass determinations with pi-mesonic atoms may have used incorrect K-shell screening corrections.      Measurements with an error of >0.005 MeV have been omitted from thi
s Listing.
DAUM 2019            10.1016/j.physletb.2019.07.027                    : 139.57021 +-0.00014 MeV
TRASSINELLI 2016     10.1016/j.physletb.2016.06.025                    : 139.57077 +-0.00018 MeV
LENZ 1998            10.1016/S0370-2693(97)01337-3                     : 139.57071 +-0.00053 MeV
JECKELMANN 1994      10.1016/0370-2693(94)90358-1                      : 139.56995 +-0.00035 MeV
ASSAMAGAN 1996       10.1103/PhysRevD.53.6065                          : 139.57022 +-0.00014 MeV
JECKELMANN 1994      10.1016/0370-2693(94)90358-1                      : 139.56782 +-0.00037 MeV
DAUM 1991            10.1016/0370-2693(91)90078-5                      : 139.56996 +-0.00067 MeV
JECKELMANN 1986B     10.1016/0375-9474(86)90476-8                      : 139.56752 +-0.00037 MeV
ABELA 1984           10.1016/0370-2693(84)90155-2                      : 139.5704 +-0.0011 MeV
LU 1980              10.1103/PhysRevLett.45.1066                       : 139.5664 +-0.0009 MeV
CARTER 1976          10.1103/PhysRevLett.37.1380                       : 139.5686 +-0.0020 MeV
MARUSHENKO 1976                                                        : 139.5660 +-0.0024 MeV
```
We are sometimes limited by the type schema defined by the PDG, but this is just a matter of safety. As you can see, the last reference here didn't come with a DOI. However, switching `doi` for `inspire_id` does give us a result for every entry here. These measurement entries also include additional information like systematic uncertainties, event counts, and experimental details, all found in the PDG database.

### Decays
Branching ratios/fractions are also available in the database, and `pdg-rs` provides an interface for exploring them:
```rust
let decays = pi_plus.branching_fractions()?;
for decay in decays {
    println!("{:<20} {}", decay.value.to_string(), decay.description)
}
```
```text
99.98770+-0.00004%   pi+ --> mu+ nu_mu
(1.230+-0.004)E-4    pi+ --> e+ nu_e
(1.036+-0.006)E-8    pi+ --> e+ nu_e pi0
(3.2+-0.5)E-9        pi+ --> e+ nu_e e+ e-
<9E-6                pi+ --> mu+ nu_mu nu nubar
<1.6E-7              pi+ --> e+ nu_e nu nubar
<1.5E-3              pi+ --> mu+ nubar_e
<8.0E-3              pi+ --> mu+ nu_e
<1.6E-6              pi+ --> mu- e+ e+ nu
```
Of course, we might like to see how these measurements were obtained. Sometimes we may be able to find a raw branching fraction measurement, but more often these are associated to measurements of branching ratios. We can query branching ratios, but we can also find related data entries):
```rust
let decays = pi_plus.branching_fractions()?;
for decay in decays {
    println!("{:<20} {}", decay.value.to_string(), decay.description);
    for data in &decay.related_data {
        println!("    {:<20} {}", data.value.to_string(), data.description);
        for measurement in data.measurements()? {
            println!(
                "        {:<25}  {}",
                measurement.reference.document_id,
                measurement
                    .reference
                    .doi
                    .clone()
                    .or(measurement.reference.inspire_id.clone())
                    .or(measurement.reference.title.clone())
                    .unwrap_or_default()
            );
        }
    }
}
```
```text
99.98770+-0.00004%   pi+ --> mu+ nu_mu
    1.2327+-0.0023E-4    [G(pi+ --> e+ nu_e)+ G(pi+ --> e+ nu(e) gamma)]/[G(pi+ --> mu+ nu_mu)+ G(pi+ --> mu+ nu_mu gamma)]
        AGUILAR-AREVALO 2015       10.1103/PhysRevLett.115.071801
        CZAPEK 1993                10.1103/PhysRevLett.70.17
        BRITTON 1992               10.1103/PhysRevLett.68.3000
        BRYMAN 1986                10.1103/PhysRevD.33.1211
        DICAPUA 1964               10.1103/PhysRev.133.B1333
        ANDERSON 1960              10.1103/PhysRev.119.2050
    3.2+-0.5E-9          G(pi+ --> e+ nu(e) e+ e-)/G(pi+ --> mu+ nu_mu)
        EGLI 1989                  10.1016/0370-2693(89)90358-4
        BARANOV 1992               
        KORENCHENKO 1976B          
        KORENCHENKO 1971           
    <8.6E-6              G(pi+ --> mu+ nu_mu nu nubar)/G(pi+ --> mu+ nu_mu)
        AGUILAR-AREVALO 2020A      10.1103/PhysRevD.102.012001
(1.230+-0.004)E-4    pi+ --> e+ nu_e
    (123.0+-0.4)E-2E-4   G(pi+ --> e+ nu_e)/G(total)
    1.2327+-0.0023E-4    [G(pi+ --> e+ nu_e)+ G(pi+ --> e+ nu(e) gamma)]/[G(pi+ --> mu+ nu_mu)+ G(pi+ --> mu+ nu_mu gamma)]
        AGUILAR-AREVALO 2015       10.1103/PhysRevLett.115.071801
        CZAPEK 1993                10.1103/PhysRevLett.70.17
        BRITTON 1992               10.1103/PhysRevLett.68.3000
        BRYMAN 1986                10.1103/PhysRevD.33.1211
        DICAPUA 1964               10.1103/PhysRev.133.B1333
        ANDERSON 1960              10.1103/PhysRev.119.2050
(1.036+-0.006)E-8    pi+ --> e+ nu_e pi0
    1.036+-0.006E-8      G(pi+ --> e+ nu(e) pi0)/G(total)
        POCANIC 2004               10.1103/PhysRevLett.93.181803
        MCFARLANE 1985             10.1103/PhysRevD.32.547
        DEPOMMIER 1968             10.1016/0550-3213(68)90305-2
        BACASTOW 1965              10.1103/PhysRev.139.B407
        BERTRAM 1965               10.1103/PhysRev.139.B617
        DUNAITSEV 1965             
        BARTLETT 1964              10.1103/PhysRev.136.B1452
        DEPOMMIER 1963             10.1016/S0375-9601(63)80030-4
(3.2+-0.5)E-9        pi+ --> e+ nu_e e+ e-
    3.2+-0.5E-9          G(pi+ --> e+ nu(e) e+ e-)/G(pi+ --> mu+ nu_mu)
        EGLI 1989                  10.1016/0370-2693(89)90358-4
        BARANOV 1992               
        KORENCHENKO 1976B          
        KORENCHENKO 1971           
<9E-6                pi+ --> mu+ nu_mu nu nubar
    <8.6E-6              G(pi+ --> mu+ nu_mu nu nubar)/G(pi+ --> mu+ nu_mu)
        AGUILAR-AREVALO 2020A      10.1103/PhysRevD.102.012001
<1.6E-7              pi+ --> e+ nu_e nu nubar
    <1.6E-7              G(pi+ --> e+ nu(e) nu nubar)/G(total)
        AGUILAR-AREVALO 2020A      10.1103/PhysRevD.102.012001
        PICCIOTTO 1988             10.1103/PhysRevD.37.1131
<1.5E-3              pi+ --> mu+ nubar_e
    <1.5E-3              G(pi+ --> mu+ nubar_e)/G(total)
        COOPER 1982                10.1016/0370-2693(82)90914-5
<8.0E-3              pi+ --> mu+ nu_e
    <8.0E-3              G(pi+ --> mu+ nu_e)/G(total)
        COOPER 1982                10.1016/0370-2693(82)90914-5
<1.6E-6              pi+ --> mu- e+ e+ nu
    <1.6E-6              G(pi+ --> mu- e+ e+ nu)/G(total)
        BARANOV 1991B              
        KORENCHENKO 1987
```

### Particle searches
It is often useful to search for all particles which satisfy some set of properties. This is currently sort of difficult to do with just the PDG website or other programmatic interfaces, but generally enough information exists in the database itself to allow for complex searches. For example, let's search for all particles which may decay to $K_S^0K_S^0$. By default, searches are inclusive, so $K_S^0$ will be mapped to other entries like $K^0$ and $\bar{K}^0$, which is useful since these are often the states listed in the database. Of course, not everything that can decay to $K\bar{K}$ can also decay to $K_S^0K_S^0$, and the PDG sometimes includes forbidden decays simply because there are published results, so it's often nice to narrow the search with additional information, such as $P$, $C$, and charge:
```rust
use pdg_rs::{Charge, Parity, ParticleSearchQuery, Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open()?;
    let particles = pdg.search_particles(
        ParticleSearchQuery::default()
            .decays_to(["K(S)0", "K(S)0"])
            .charge(Charge::Neutral)
            .parity(Parity::Plus)
            .charge_conjugation(Parity::Plus),
    )?;
    for particle in particles {
        println!("{particle}");
    }
    Ok(())
}
```
```text
f_0(980)0 (M003, Meson, Self-Conjugate, charge 0), MCID 9010221, I=0, G=+, J=0, P=+, C=+
f_2(1270)0 (M005, Meson, Self-Conjugate, charge 0), MCID 225, I=0, G=+, J=2, P=+, C=+
a_2(1320)0 (M012, Meson, Self-Conjugate, charge 0), MCID 115, I=1, G=-, J=2, P=+, C=+
f_2^'(1525)0 (M013, Meson, Self-Conjugate, charge 0), MCID 335, I=0, G=+, J=2, P=+, C=+
f_4(2050)0 (M016, Meson, Self-Conjugate, charge 0), MCID 229, I=0, G=+, J=4, P=+, C=+
a_4(1970)0 (M017, Meson, Self-Conjugate, charge 0), MCID 119, I=1, G=-, J=4, P=+, C=+
a_0(980)0 (M036, Meson, Self-Conjugate, charge 0), MCID 9000111, I=1, G=-, J=0, P=+, C=+
f_4(2300)0 (M041, Meson, Self-Conjugate, charge 0), MCID 9010229, I=0, G=+, J=4, P=+, C=+
f_2(2150)0 (M042, Meson, Self-Conjugate, charge 0), MCID 9070225, I=0, G=+, J=2, P=+, C=+
chi_c2(3930) (M050, Meson, Self-Conjugate, charge 0), MCID 100445, I=0, G=+, J=2, P=+, C=+
chi_c1(1P) (M055, Meson, Self-Conjugate, charge 0), MCID 20443, I=0, G=+, J=1, P=+, C=+
chi_c0(1P) (M056, Meson, Self-Conjugate, charge 0), MCID 10441, I=0, G=+, J=0, P=+, C=+
chi_c2(1P) (M057, Meson, Self-Conjugate, charge 0), MCID 445, I=0, G=+, J=2, P=+, C=+
f_2(1430)0 (M066, Meson, Self-Conjugate, charge 0), MCID 9000225, I=0, G=+, J=2, P=+, C=+
f_0(1710)0 (M068, Meson, Self-Conjugate, charge 0), MCID 10331, I=0, G=+, J=0, P=+, C=+
f_J(2220)0 (M082, Meson, Self-Conjugate, charge 0), MCID 9000229, I=0, G=+, J=2++ or 4, P=+, C=+
f_2(2010)0 (M106, Meson, Self-Conjugate, charge 0), MCID 9060225, I=0, G=+, J=2, P=+, C=+
f_2(2300)0 (M107, Meson, Self-Conjugate, charge 0), MCID 9080225, I=0, G=+, J=2, P=+, C=+
f_2(1640)0 (M117, Meson, Self-Conjugate, charge 0), MCID 9020225, I=0, G=+, J=2, P=+, C=+
f_2(1565)0 (M123, Meson, Self-Conjugate, charge 0), MCID 9010225, I=0, G=+, J=2, P=+, C=+
f_2(1950)0 (M135, Meson, Self-Conjugate, charge 0), MCID 9050225, I=0, G=+, J=2, P=+, C=+
f_2(1910)0 (M142, Meson, Self-Conjugate, charge 0), MCID 9040225, I=0, G=+, J=2, P=+, C=+
f_0(1370)0 (M147, Meson, Self-Conjugate, charge 0), MCID 10221, I=0, G=+, J=0, P=+, C=+
a_0(1450)0 (M149, Meson, Self-Conjugate, charge 0), MCID 10111, I=1, G=-, J=0, P=+, C=+
f_0(1500)0 (M152, Meson, Self-Conjugate, charge 0), MCID 9030221, I=0, G=+, J=0, P=+, C=+
chi_c0(3915) (M159, Meson, Self-Conjugate, charge 0), I=0, G=+, J=0, P=+, C=+
a_2(1700)0 (M162, Meson, Self-Conjugate, charge 0), MCID 9000115, I=1, G=-, J=2, P=+, C=+
a_0(1950)0 (M227, Meson, Self-Conjugate, charge 0), I=1, G=-, J=0, P=+, C=+
a_0(1710)0 (M263, Meson, Self-Conjugate, charge 0), I=1, G=-, J=0, P=+, C=+
f_0(1770)0 (M264, Meson, Self-Conjugate, charge 0), I=0, G=+, J=0, P=+, C=+
```

There are lots of interesting ways to filter particle searches, such as selecting particles in a certain mass range.

The database itself contains data beyond individual particle and decay measurements. A lot of this data is hard to classify, so it's often useful to use search queries. For example, if we wanted to learn about constraints on extra dimensions, we might do the following:
```rust
let search_results = pdg.search_text("extra dimensions Newtonian")?;
for result in search_results {
    println!("{}\n", result.text);
    let measurements = pdg.measurements_for(result.pdg_id)?;
    for measurement in &measurements {
        println!(
            "{:<25} ({})",
            measurement.reference.document_id,
            measurement.comment.clone().unwrap_or_default()
        );
        for value in &measurement.values {
            if value.value.is_some() {
                println!("Value: {}", value);
            }
            for footnote in &measurement.footnotes {
                println!("{}", footnote.text.clone().unwrap_or_default());
            }
        }
        println!();
    }
    println!("\nReferences:\n");
    for measurement in measurements {
        println!(
            "{:<25}  {}",
            measurement.reference.document_id,
            measurement
                .reference
                .doi
                .clone()
                .or(measurement.reference.inspire_id.clone())
                .or(measurement.reference.title.clone())
                .unwrap_or_default()
        );
    }
}
```
```text
This section includes limits on the size of extra dimensions from deviations in the Newtonian (1/r**2) gravitational force law at short distances. Deviations are parametrized
 by a gravitational potential of the form V = -(G m m'/r) [1 + alpha exp(-r/R)]. For delta toroidal extra dimensions of equal size, alpha = 8delta/3. Quoted bounds are for de
lta = 2 unless otherwise noted.

BLAKEMORE 2021            (Optical levitation)
BLAKEMORE 2021 obtain constraints on non-Newtonian forces with strengths |alpha| ~> E8 and length scales R > 10 mum. See their Fig. 4 for more details including comparison wi
th previous searches.

HEACOCK 2021              (Neutron scattering)
HEACOCK 2021 obtain constraints on non-Newtonian forces with strengths E18~< |alpha|~< E25 and length scales R ~= 0.02 -- 10 nm. See their Figure 3 for more details. This imp
roves the results of HADDOCK 2018. These constraints do not place limits on the size of extra flat dimensions.

LEE 2020                  (Torsion pendulum)
LEE 2020 search for new forces probing a range of |alpha| ~=  and length scales R ~= 7 -- 90 mum. For delta = 1 the bound on R is 30 mum. See their Fig. 5 for details on the 
bound.

TAN 2020A                 (Torsion pendulum)
Value: <37 micrometers
TAN 2020A search for new forces probing a range of |alpha| ~= 4E-3 -- 1E2 and length scales R ~= 40 -- 350 mum. See their Fig. 6 for details on the bound.

BERGE 2018                (Space accelerometer)
BERGE 2018 uses results from the MICROSCOPE experiment to obtain constraints on non-Newtonian forces with strengths E-11~< |alpha|~< E-7 and length scales R ~>E5 m. See their
 Figure 1 for more details. These constraints do not place limits on the size of extra flat dimensions.

FAYET 2018A               (Space accelerometer)
FAYET 2018A uses results from the MICROSCOPE experiment to obtain constraints on an EP-violating force possibly arising from a new U(1) gauge boson. For R ~>E7 m the limits a
re |alpha| ~< a few E-13 to a few E-11 depending on the coupling, corresponding to |epsilon| ~< E-24 for the coupling of the new spin-1 or spin-0 mediator. These constraints 
do not place limits on the size of extra flat dimensions. This extends the results of FAYET 2018.

KLIMCHITSKAYA 2017A       (Torsion oscillator)
KLIMCHITSKAYA 2017A uses an experiment that measures the difference of Casimir forces to obtain bounds on non-Newtonian forces with strengths |alpha| ~= E5 -- E17 and length 
scales R = 0.03 -- 10 mum. See their Fig. 3. These constraints do not place limits on the size of extra flat dimensions.

XU 2013                   (Nuclei properties)
XU 2013 obtain constraints on non-Newtonian forces with strengths |alpha| ~= 10**34 -- 10**36 and length scales R ~= 1 -- 10 fm. See their Fig. 4 for more details. These cons
traints do not place limits on the size of extra flat dimensions.

BEZERRA 2011              (Torsion oscillator)
BEZERRA 2011 obtain constraints on non-Newtonian forces with strengths E11~< |alpha|~< E18 and length scales R = 30 -- 1260 nm. See their Fig. 2 for more details. These const
raints do not place limits on the size of extra flat dimensions.

SUSHKOV 2011              (Torsion pendulum)
SUSHKOV 2011 obtain improved limits on non-Newtonian forces with strengths E7~< |alpha| ~< E11 and length scales 0.4 mum < R < 4 mum (95% CL). See their Fig. 2. These bounds 
do not place limits on the size of extra flat dimensions. However, a model dependent bound of M_{{*}} > 70 TeV is obtained assuming gauge bosons that couple to baryon number 
also propagate in (4 + delta) dimensions.

BEZERRA 2010              (Microcantilever)
BEZERRA 2010 obtain improved constraints on non-Newtonian forces with strengths E19~< |alpha|~< E29 and length scales R = 1.6 -- 14 nm (95% CL). See their Fig. 1. This bound 
does not place limits on the size of extra flat dimensions.

MASUDA 2009               (Torsion pendulum)
MASUDA 2009 obtain improved constraints on non-Newtonian forces with strengths E9~<|alpha|~<E11 and length scales R = 1.0 -- 2.9 mum (95% CL). See their Fig. 3. This bound do
es not place limits on the size of extra flat dimensions.

GERACI 2008               (Microcantilever)
GERACI 2008 obtain improved constraints on non-Newtonian forces with strengths |alpha| > 14,000 and length scales R = 5 -- 15 micrometers. See their Fig. 9. This bound does n
ot place limits on the size of extra flat dimensions.

TRENKEL 2008              (Newton's constant)
TRENKEL 2008 uses two independent measurements of Newton's constant G to constrain new forces with strength |alpha|~=E-4 and length scales R = 0.02 -- 1 m. See their Fig. 1. 
This bound does not place limits on the size of extra flat dimensions.

DECCA 2007A               (Torsion oscillator)
DECCA 2007A search for new forces and obtain bounds in the region with strengths |alpha| ~= E13 -- E18 and length scales R = 20 -- 86 nm. See their Fig. 6. This bound does no
t place limits on the size of extra flat dimensions.

KAPNER 2007               (Torsion pendulum)
Value: <37 micrometers
KAPNER 2007 search for new forces, probing a range of |alpha| ~= 10**-3 -- 10**5 and length scales R ~= 10 -- 1000 mum. For delta = 1 the bound on R is 44 mum. For delta = 2,
 the bound is expressed in terms of M_{{*}}, here translated to a bound on the radius. See their Fig. 6 for details on the bound.

TU 2007                   (Torsion pendulum)
Value: <47 micrometers
TU 2007 search for new forces probing a range of |alpha| ~= E-1 -- E5 and length scales R ~= 20 -- 1000 mum. For delta = 1 the bound on R is 53 mum. See their Fig. 3 for deta
ils on the bound.

SMULLIN 2005              (Microcantilever)
SMULLIN 2005 search for new forces, and obtain bounds in the region with strengths alpha ~= 10**3 -- 10**8 and length scales R = 6 -- 20 mum. See their Figs. 1 and 16 for det
ails on the bound. This work does not place limits on the size of extra flat dimensions.

HOYLE 2004                (Torsion pendulum)
Value: <130 micrometers
HOYLE 2004 search for new forces, probing alpha down to 10**-2 and distances down to 10mum. Quoted bound on R is for delta = 2. For delta = 1, bound goes to 160 mum. See thei
r Fig. 34 for details on the bound.

CHIAVERINI 2003           (Microcantilever)
CHIAVERINI 2003 search for new forces, probing alpha above 10**4 and lambda down to 3mum, finding no signal. See their Fig. 4 for details on the bound. This bound does not pl
ace limits on the size of extra flat dimensions.

LONG 2003                 (Microcantilever)
LONG 2003 search for new forces, probing alpha down to 3, and distances down to about 10mum. See their Fig. 4 for details on the bound.

HOYLE 2001                (Torsion pendulum)
Value: <190 micrometers
HOYLE 2001 search for new forces, probing alpha down to 10**-2 and distances down to 20mum. See their Fig. 4 for details on the bound. The quoted bound is for alpha >= 3.

HOSKINS 1985              (Torsion pendulum)
HOSKINS 1985 search for new forces, probing distances down to 4 mm. See their Fig. 13 for details on the bound. This bound does not place limits on the size of extra flat dim
ensions.

BLAKEMORE 2021             10.1103/PhysRevD.104.L061101
HEACOCK 2021               10.1126/science.abc2794
LEE 2020                   10.1103/PhysRevLett.124.101101
TAN 2020A                  10.1103/PhysRevLett.124.051301
BERGE 2018                 10.1103/PhysRevLett.120.141101
FAYET 2018A                10.1103/PhysRevD.99.055043
KLIMCHITSKAYA 2017A        10.1103/PhysRevD.95.123013
XU 2013                    10.1088/0954-3899/40/3/035107
BEZERRA 2011               10.1103/PhysRevD.83.075004
SUSHKOV 2011               10.1103/PhysRevLett.107.171101
BEZERRA 2010               10.1103/PhysRevD.81.055003
MASUDA 2009                10.1103/PhysRevLett.102.171101
GERACI 2008                10.1103/PhysRevD.78.022002
TRENKEL 2008               10.1103/PhysRevD.77.122001
DECCA 2007A                10.1140/epjc/s10052-007-0346-z
KAPNER 2007                10.1103/PhysRevLett.98.021101
TU 2007                    10.1103/PhysRevLett.98.201101
SMULLIN 2005               10.1103/PhysRevD.72.122001
HOYLE 2004                 10.1103/PhysRevD.70.042004
CHIAVERINI 2003            10.1103/PhysRevLett.90.151101
LONG 2003                  10.1038/nature01432
HOYLE 2001                 10.1103/PhysRevLett.86.1418
HOSKINS 1985               10.1103/PhysRevD.32.3084
```

Data is not always in a consistent form, although that is mostly due to the difficulty of having to account for so many different kinds of data entries. The search feature is intended to be more of a backend to a future CLI/TUI which will allow for these sorts of resources to be discovered in a more interactive manner rather than poking around at each `pdg_id` entry to see what references/footnotes/values it may relate to.

## Project Status
Currently this is just a working example. I have some plans to expand the interface, but I'm open to outside contributions and ideas, please raise issues or make pull requests if interested. Some things I'm currently considering/working on:

- A CLI/TUI interface for handling particle queries and raw data searches with better organization of footnotes and references.
- Rust structs mirroring every table to provide a consistent backend
- Incorporating the JSON data from [here](https://pdg.lbl.gov/current/PDGIdentifiers.json) and [here](https://pdg.lbl.gov/current/PDGIdentifiers-references.json) to add a bit of additional context to the database.
- Automatically downloading and/or dynamically updating the database when a new version of the PDG is released.
- Possibly a Python interface, although I'd have to ensure there is some actual value to it beyond a clone of the [`particle`](https://pypi.org/project/particle/) library. I think right now it could be justified given that `particle` is mainly a curated selection raw particle information while `pdg-rs` incorporates the entire database of measurements and data not specific to any single particle.

## Citations
When using data from `pdg-rs`, please cite:
```
@article{ParticleDataGroup:2024cfk,
    author = "Navas, S. and others",
    collaboration = "Particle Data Group",
    title = "{Review of particle physics}",
    doi = "10.1103/PhysRevD.110.030001",
    journal = "Phys. Rev. D",
    volume = "110",
    number = "3",
    pages = "030001",
    year = "2024"
}
```


## See also:
The [`particle`](https://pypi.org/project/particle/) library (Python)
