## ADDED Requirements

### Requirement: About Panel States Product and Format

The macOS About panel SHALL be a governed brand surface. It SHALL identify the application as "SpecForge", display the application version and copyright, and its credits text SHALL name the **OpenSpec** format the application reads — keeping both names in their correct senses per the brand-vs-format distinction (SpecForge is the product; OpenSpec is the format). The tagline line carried in the credits text SHALL be consistent with the bundle short description rather than introducing a divergent product description.

#### Scenario: About panel identifies the product as SpecForge

- **WHEN** the user opens the macOS About panel
- **THEN** the panel displays the product name as "SpecForge"
- **AND** it displays the application version and the copyright line

#### Scenario: About panel names the OpenSpec format

- **WHEN** the About panel's credits text is shown
- **THEN** it contains a tagline that refers to the **OpenSpec** format the application reads
- **AND** it uses "OpenSpec" in its format sense, not as a product name

#### Scenario: Tagline consistent with the bundle description

- **WHEN** the tagline line in the About panel's credits text and the bundle short description are compared
- **THEN** they describe the product consistently (the tagline does not introduce a divergent product description)
