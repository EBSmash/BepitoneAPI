import PIL
from PIL import Image
import requests
from io import BytesIO
from PIL import ImageFilter
from PIL import ImageEnhance
from IPython.display import display
import numpy as np

URL = "https://i.ibb.co/q1fNPXG/New-Piskel.png"
response = requests.get(URL)
img = Image.open(BytesIO(response.content))
 
# get the mode of the image
img.mode 
 
# create a grayscale image
grayimg = img.convert('L')
grayimg.mode

img.getpixel((0,0))
grayimg.getpixel((0,0))

def binarize(image_to_transform, threshold):
    # now, lets convert that image to a single greyscale image using convert()
    output_image=image_to_transform.convert("L")

    for x in range(output_image.width):
        f = open("{}".format(x) , "w")
        for y in range(output_image.height):
            # for the given pixel at w,h, lets check its value against the threshold
            if output_image.getpixel((x,y))< threshold: #note that the first parameter is actually a tuple object
                # lets set this to zero
                output_image.putpixel( (x,y), 0 )
                f.write("{} {}\n".format(x,y))
            else:
                # otherwise lets set this to 255
                output_image.putpixel( (x,y), 255 )
    #now we just return the new image
    return output_image
 

binarize(img, 75).save("popbob.png", "PNG")