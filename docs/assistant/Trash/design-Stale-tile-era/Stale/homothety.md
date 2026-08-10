A homothety is made of two IntExp values (the loaction) and one i32 zoom magnification POT.
In order to be valid, the magnification POT plus the pixels per unit POT must match the inverse of the location exponents. This is because homotheties only contain as much precision as is necessary for their magnification level. Otherwise, they would infinitely collect precision with every zoom in-out.
