---
company:
  name: Aperture Science, Inc.
  shortname: Aperture Science
  address:
    street: 2300 Port Hill Rd.
    city: Gulliver
    state: MI
  representative:
    name: Caroline Johnson
    position: Sales Manager
    email: caroline.johnson@aperturescience.com
customers:
  - title: Mr.
    first: P.
    last: Sherman
    organization: Sydney Orthodontics
    address:
      street: 42 Wallaby Way
      city: Sydney
      state: NSW
    orders:
      - name: fish-shaped ethylbenzene
        unit: 55-gallon drums
        qty: 9
        value: 1238.19
        paid: true
      - name: fish-shaped volatile organic compounds
        unit: containers
        qty: 3
        value: 46.20
        paid: false
      - name: unsaturated polyester resin
        unit: can
        qty: 1
        value: 24.00
        paid: false
      - name: fiberglass surface resins
        unit: cans
        qty: 2
        value: 15.79
        paid: true
      - name: volatile malted milk impoundments
        unit: pounds
        qty: 3
        value: 18.89
        paid: true
      - name: medium geosynthetic membranes
        qty: 12
        value: 922.36
        paid: false
      - name: cross-borehole electromagnetic imaging rhubarb
        qty: 1
        value: 23201.66
        paid: false
      - name: adjustable aluminum head positioners
        qty: 2
        value: 38.21
        paid: false
      - name: cordless electric needle injector
        qty: 1
        value: 78.04
        paid: true
      - name: injector needle driver
        qty: 1
        value: 46.51
        paid: false
      - name: injector needle gun
        qty: 1
        value: 39.94
        paid: false
      - name: cranial caps
        qty: 20
        value: 479.17
        paid: false
...
{{#customers}}{{!
}}From:
    {{.company.name}}
    {{.company.address.street}}
    {{.company.address.city}}, {{.company.address.state}}
To:
    {{title}} {{first}} {{last}}
    {{organization}}
    {{address.street}}
    {{address.city}}, {{address.state}}

Dear {{title}} {{last}},

Thank you for being a valued customer of {{.company.shortname}}.  We are writing to inform you that according to our records, there are some items you purchased from us for which payment is outstanding.  The missing items are as follows:

{{#orders}}{{^paid}}  - {{&.qty}}{{#&.unit}} {{}}{{/}} {{&.name}} (${{&.value}})
{{/}}{{/}}
We have already received your payment for the following orders:

{{#orders}}{{#paid}}  - {{&.qty}}{{#&.unit}} {{}}{{/}} {{&.name}}
{{/}}{{/}}
Please respond promptly with all outstanded payments to avoid penalty charges on your account.  If there are any issues with the orders shown here, please contact us at {{.company.representative.email}} as soon as possible so we can resolve them.  We appreciate your prompt attention to this matter.

Sincerely,
{{.company.representative.name}}
{{.company.representative.position}}
{{.company.name}}{{/}}
